use super::*;

#[test]
fn plan_request_rejects_unknown_field() {
    assert!(
        serde_json::from_value::<PlanRequest>(serde_json::json!({"path": "a", "x": 1})).is_err()
    );
}

#[test]
fn run_step_rejects_unknown_field() {
    let v = serde_json::json!({"relPath": "a", "destDir": "", "x": 1});
    assert!(serde_json::from_value::<RunStep>(v).is_err());
}

#[test]
fn step_result_json_shape() {
    let w = WriteFileResponse {
        path: "p".into(),
        hash: "h".into(),
        warnings: vec![],
        contains_escape: false,
    };
    assert_eq!(
        serde_json::to_value(StepResult::written("a".into(), w)).unwrap(),
        serde_json::json!({"relPath": "a", "status": "written", "path": "p", "hash": "h",
            "warnings": [], "containsEscape": false})
    );
    let e = OpError::new(error::FILE_EXISTS).with("path", "p");
    assert_eq!(
        serde_json::to_value(StepResult::skipped("a".into(), e)).unwrap(),
        serde_json::json!({"relPath": "a", "status": "skipped", "code": "file.exists",
            "params": {"path": "p"}})
    );
}

#[test]
fn batch_counts() {
    let b = BatchResponse::new(vec![
        StepResult::skipped("a".into(), OpError::new(error::FILE_EXISTS)),
        StepResult::skipped("b".into(), OpError::new(error::FILE_EXISTS)),
    ]);
    assert_eq!((b.written, b.skipped, b.results.len()), (0, 2, 2));
}
