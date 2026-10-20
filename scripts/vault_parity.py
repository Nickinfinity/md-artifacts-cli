#!/usr/bin/env python3
"""End-to-end parity of the two ways a client drives `mda`: JSON-RPC (`mda serve`) and the CLI.

Each leg gets its own copy of the vault, its own workspace and its own `MDA_CONFIG_DIR` (the real
vault and the user's config are never touched). The same scenario runs on both legs, per source
artifact under the scope:

  read -> create a copy from the read data -> varsets.match -> varsets.apply each match
  -> render (and write_file for whole-file types) with those values; Variables files: read ->
  create a copy -> apply each sub-set -> varsets.save its values.

Checks (any failure exits 1):
  rpc == cli         every response, and the resulting vault + workspace trees, byte for byte
  render(copy) == render(source), per var set: the inserted text survives create(read(S))
  fixpoint           create(read(copy)) writes the copy's exact bytes (the emitted form is stable)
Notes (reported, never a failure; full text in DIR/notes.txt):
  copy != source     the file is not in canonical layout (spacing, key order, list style: spec §7.6);
                     one `artifact.update` with the read model converges it
  create refused     a read model the emitter cannot write back (e.g. `## ` or a fence in a
                     flagged payload: spec §2 "an emitter never writes `## ` inside code")

Usage:
  python3 -I scripts/vault_parity.py --vault ~/N0t3s/C0d3-Sn1pp3ts [--scope test] [--out DIR]
Exit 0 = all checks pass, 1 = a check failed. Every response is kept under DIR/<leg>/responses/,
so `diff -r DIR/rpc DIR/cli` shows any divergence.
"""
import argparse
import difflib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PARITY = "parity"  # sub-folder (inside each type dir) the copies are created in


class Rpc:
    """A minimal NDJSON JSON-RPC client over `mda serve`'s stdio."""

    name = "rpc"

    def __init__(self, mda, vault, env):
        self.p = subprocess.Popen([mda, "serve", "--vault", str(vault)], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, env=env, text=True)
        self.next_id = 0
        self.init = self._send("initialize", {"protocol": "2.0", "clientInfo": {"name": "parity"}})

    def _send(self, method, params):
        self.next_id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.next_id, "method": method,
                                       "params": params}) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        msg = json.loads(line)  # a non-JSON stdout line is itself a protocol failure
        assert msg["id"] == self.next_id, msg
        if "error" in msg:
            return {"error": msg["error"]["data"]}
        return msg["result"]

    def op(self, method, params):
        return self._send(method, params)

    def close(self):
        self.p.stdin.close()
        assert self.p.wait(10) == 0, "serve exited non-zero"


class Cli:
    """The same ops through the human subcommands, `--json` for the exact response."""

    name = "cli"

    def __init__(self, mda, vault, env, tmp):
        self.mda, self.vault, self.env, self.tmp, self.n = mda, str(vault), env, tmp, 0

    def _file(self, obj):
        self.n += 1
        f = self.tmp / f"arg{self.n}.json"
        f.write_text(json.dumps(obj))
        return str(f)

    def _argv(self, method, p):
        if method == "artifact.read":
            return ["artifact", "show", p["path"]]
        if method == "artifact.create":
            return ["artifact", "new", p["path"], "--model", self._file(p["model"])]
        if method == "varsets.match":
            return ["varsets", "match", p["path"], *blk(p)]
        if method == "varsets.apply":
            s = p["subset"]
            return ["varsets", "apply", p["path"], "--subset", str(s["index"]),
                    "--heading", s["heading"], "--values", self._file(p.get("values", {}))]
        if method == "artifact.render":
            return ["artifact", "render", p["path"], *blk(p), "--values", self._file(p["values"])]
        if method == "artifact.write_file":
            return ["artifact", "write-file", p["path"], "--workspace", p["workspaceRoot"],
                    "--dest", p["destDir"], *blk(p), "--values", self._file(p["values"])]
        if method == "varsets.save":
            tags = [a for t in p.get("tags", []) for a in ("--tag", t)]
            return ["varsets", "save", p["title"], "--dir", p["dir"], "--description",
                    p.get("description", ""), *tags, "--values", self._file(p["values"])]
        raise KeyError(method)

    def op(self, method, params):
        argv = [self.mda, *self._argv(method, params), "--vault", self.vault, "--json"]
        r = subprocess.run(argv, capture_output=True, text=True, env=self.env)
        if r.returncode not in (0, 1):
            raise RuntimeError(f"{argv}: exit {r.returncode}: {r.stderr}")
        body = json.loads(r.stdout)
        return {"error": body} if r.returncode == 1 else body

    def close(self):
        pass


def blk(p):
    return [] if p.get("block") is None else ["--block", str(p["block"])]


def to_model(parsed):
    """Client-side `parsedToModel` (mirrors `mda_core::serialize::from_parsed`)."""
    fm = parsed["frontmatter"]
    variables = fm["artifactType"] == "Variables"
    var = lambda v: {"name": v["name"], "value": v.get("value", v["defaultValue"])}
    if parsed["blocks"]:
        blocks = [{"heading": b["heading"], "description": b["description"],
                   "language": "" if variables else (b.get("fenceLang") or ""),
                   "code": "" if variables else b["code"], "vars": [var(v) for v in b["vars"]]}
                  for b in parsed["blocks"]]
    elif variables:
        blocks = [{"vars": [var(v) for v in parsed["vars"]]}] if parsed["vars"] else []
    else:
        blocks = [{"language": fm.get("language") or "", "code": parsed["code"],
                   "vars": [var(v) for v in parsed["vars"]]}]
    keys = ("title", "description", "extension", "provider", "model", "version", "env", "target")
    m = {"artifactType": fm["artifactType"], **{k: fm.get(k) or "" for k in keys},
         "tags": fm.get("tags") or [], "blocks": blocks}
    if fm.get("index") is True:
        m["index"] = True
    if fm.get("paths"):
        m["paths"] = fm["paths"]
    if parsed.get("layout"):
        m["layout"] = parsed["layout"]  # flags / bare body, with notes and chrome (protocol 1.1)
    return m


class Leg:
    def __init__(self, client, root):
        self.c, self.root, self.log = client, root, {}

    def op(self, step, method, params):
        res = self.c.op(method, params)
        self.log[step] = {"method": method, "params": params, "response": res}
        return res


def scenario(leg, sources, types, ws):
    """The client flow; returns (hard failures, notes) specific to this leg."""
    fails, notes = [], []
    for src in sources:
        tdir, rel = src.split("/", 1)
        copy = f"{tdir}/{PARITY}/{rel}"
        parsed = leg.op(f"{src} | read", "artifact.read", {"path": src})
        if "error" in parsed:
            continue
        made = leg.op(f"{src} | create", "artifact.create", {"path": copy, "model": to_model(parsed)})
        if "error" in made:
            notes.append(("create refused", src, json.dumps(made["error"])))
            copy = src  # nothing to compare against; the rest of the flow still runs
        else:
            a, b = (leg.root / "vault" / p for p in (src, copy))
            if a.read_bytes() != b.read_bytes():
                notes.append(("copy != source", src, udiff(a.read_text(), b.read_text())))
            fails += fixpoint(leg, src, copy)
        t = types[tdir]
        if t["type"] == "Variables":
            if not parsed["blocks"] and not parsed["vars"]:
                continue  # blank: no sub-set to apply
            for i, blkp in enumerate(parsed["blocks"] or [{"heading": ""}]):
                sub = {"index": i, "heading": blkp["heading"]}
                ap = leg.op(f"{src} | apply {i}", "varsets.apply", {"path": src, "subset": sub})
                if "error" in ap:
                    continue
                title = f"parity {Path(rel).stem} {i} {blkp['heading']}".strip()
                leg.op(f"{src} | save {i}", "varsets.save",
                       {"title": title, "dir": f"Variables/{PARITY}-saved",
                        "description": blkp.get("description", ""), "values": ap["values"]})
            continue
        for bi in (range(len(parsed["blocks"])) if t["multiBlock"] else [None]):
            tag = f"{src} | block {bi}"
            sets = [("defaults", {})]
            m = leg.op(f"{tag} | match", "varsets.match", {"path": src, "block": bi})
            for n, mt in enumerate([] if "error" in m else m["matches"]):
                ap = leg.op(f"{tag} | apply {mt['label']}", "varsets.apply",
                            {"path": m["paths"][mt["pathIndex"]], "subset": mt["subset"]})
                if "error" not in ap:  # labels repeat across files: the index keeps dests apart
                    sets.append((f"{n}-{mt['label']}", ap["values"]))
            for label, values in sets:
                p = {"path": src, "block": bi, "values": values}
                r1 = leg.op(f"{tag} | render {label}", "artifact.render", p)
                r2 = leg.op(f"{tag} | render copy {label}", "artifact.render", {**p, "path": copy})
                if r1 != r2:
                    fails.append(("render(copy) != render(source)", f"{tag} [{label}]",
                                  udiff(json.dumps(r1, indent=1), json.dumps(r2, indent=1))))
                if t["writesFile"]:
                    dest = f"{Path(rel).with_suffix('')}/{block_label(bi)}/{label}"
                    leg.op(f"{tag} | write_file {label}", "artifact.write_file",
                           {**p, "workspaceRoot": str(ws), "destDir": dest})
    return fails, notes


def fixpoint(leg, src, copy):
    again = copy.replace(f"/{PARITY}/", f"/{PARITY}-again/", 1)
    p = leg.op(f"{src} | read copy", "artifact.read", {"path": copy})
    r = leg.op(f"{src} | create again", "artifact.create", {"path": again, "model": to_model(p)})
    a, b = (leg.root / "vault" / x for x in (copy, again))
    if "error" in r or a.read_bytes() != b.read_bytes():
        diff = json.dumps(r) if "error" in r else udiff(a.read_text(), b.read_text())
        return [("fixpoint: create(read(copy)) != copy", src, diff)]
    return []


def block_label(bi):
    return "single" if bi is None else f"block{bi}"


def udiff(a, b):
    return "".join(difflib.unified_diff(a.splitlines(True), b.splitlines(True), "a", "b", n=1))


def tree(d):
    return {str(p.relative_to(d)): p.read_bytes() for p in sorted(d.rglob("*")) if p.is_file()}


def norm(obj, leg_root):
    """Each leg's absolute root differs; everything else must be identical."""
    return json.loads(json.dumps(obj).replace(str(leg_root), "$LEG"))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--vault", required=True, type=Path)
    ap.add_argument("--mda", default=str(ROOT / "target/debug/mda"))
    ap.add_argument("--scope", default="test", help="sub-folder of each type dir ('' = all)")
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    out = (a.out or Path(tempfile.mkdtemp(prefix="mda-parity-"))).resolve()
    if out.exists() and any(out.iterdir()):
        sys.exit(f"{out} is not empty")

    legs = []
    for name in ("rpc", "cli"):
        root = out / name
        shutil.copytree(a.vault, root / "vault", symlinks=True,
                        ignore=shutil.ignore_patterns(".git", ".DS_Store", ".trash"))
        for d in ("ws", "cfg", "tmp"):
            (root / d).mkdir()
        env = {**os.environ, "MDA_CONFIG_DIR": str(root / "cfg")}
        env.pop("MDA_LOG", None)
        c = (Rpc(a.mda, root / "vault", env) if name == "rpc"
             else Cli(a.mda, root / "vault", env, root / "tmp"))
        legs.append(Leg(c, root))
    types = {t["dir"]: t for t in legs[0].c.init["types"]}

    vroot = legs[0].root / "vault"
    sources = sorted(r for d in types if (vroot / d / a.scope).is_dir()
                     for r in (str(p.relative_to(vroot)) for p in (vroot / d / a.scope).rglob("*.md"))
                     if not r.split("/")[1].startswith(PARITY))
    if not sources:
        sys.exit(f"no .md files under <TypeDir>/{a.scope} in {a.vault}")

    fails, notes = [], []
    for leg in legs:
        f, n = scenario(leg, sources, types, leg.root / "ws")
        fails += [(leg.c.name, *x) for x in f]
        notes += [(leg.c.name, *x) for x in n if leg.c.name == "rpc"]  # same on both (rpc == cli)
        leg.c.close()

    rpc, cli = legs
    steps = 0
    for step in rpc.log.keys() | cli.log.keys():
        x, y = norm(rpc.log.get(step), rpc.root), norm(cli.log.get(step), cli.root)
        steps += 1
        if x != y:
            fails.append(("both", "rpc != cli", step,
                          udiff(json.dumps(x, indent=1), json.dumps(y, indent=1))))
    for sub in ("vault", "ws"):
        tx, ty = tree(rpc.root / sub), tree(cli.root / sub)
        for f in sorted(tx.keys() | ty.keys()):
            if tx.get(f) != ty.get(f):
                fails.append(("both", f"{sub} tree rpc != cli", f, ""))
    for leg in legs:
        d = leg.root / "responses"
        d.mkdir()
        for i, (step, rec) in enumerate(sorted(leg.log.items())):
            (d / f"{i:04}.json").write_text(json.dumps(norm(rec, leg.root), indent=1) + "\n")

    errors = sum("error" in r["response"] for r in rpc.log.values())
    (out / "notes.txt").write_text("".join(f"NOTE {k}: {w}\n{d}\n\n" for _, k, w, d in notes))
    differ = sum(k == "copy != source" for _, k, _w, _d in notes)
    print(f"{len(sources)} sources, {steps} steps per leg ({errors} op errors, compared too), "
          f"{len(fails)} failures, {len(notes)} notes\n"
          f"create(read(S)) == S byte for byte: {len(sources) - differ}/{len(sources)}\nout: {out}")
    for _, kind, where, _d in notes:
        print(f"note  {kind}: {where}")
    for leg_name, kind, where, diff in fails:
        print(f"\nFAIL [{leg_name}] {kind}: {where}\n{diff}".rstrip())
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
