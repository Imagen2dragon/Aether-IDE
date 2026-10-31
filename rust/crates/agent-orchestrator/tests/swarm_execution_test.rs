use agent_orchestrator::{
    AgentMessage, AgentMessageType, MergeResolutionAction, SwarmCoordinator, SwarmTaskNode,
    ThreeWayMergeEngine,
};

#[test]
fn test_multi_agent_dag_scheduling_and_lifecycle() {
    let temp_root =
        std::env::temp_dir().join(format!("aether_swarm_test_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_root);
    let coordinator = SwarmCoordinator::new(&temp_root);

    // 1. Task 1: Architect designs architecture
    let task_arch = SwarmTaskNode::new(
        "task-arch-01",
        "Design Subsystem Architecture",
        "Define interfaces and data structures",
        "architect",
        vec![],
    );
    coordinator.add_task(task_arch).unwrap();

    // 2. Tasks 2 & 3: Systems and Backend implement components in parallel (depend on task 1)
    let task_sys = SwarmTaskNode::new(
        "task-sys-02",
        "Implement Storage Primitives",
        "Low-level I/O routines",
        "systems",
        vec!["task-arch-01".into()],
    );
    let task_backend = SwarmTaskNode::new(
        "task-backend-03",
        "Implement RPC Services",
        "IPC and HTTP endpoints",
        "backend",
        vec!["task-arch-01".into()],
    );
    coordinator.add_task(task_sys).unwrap();
    coordinator.add_task(task_backend).unwrap();

    // 3. Task 4: QA verifies system (depends on task 2 and 3)
    let task_qa = SwarmTaskNode::new(
        "task-qa-04",
        "Integration Test & QA Validation",
        "Run end-to-end regression suites",
        "qa",
        vec!["task-sys-02".into(), "task-backend-03".into()],
    );
    coordinator.add_task(task_qa).unwrap();

    // 4. Initial Ready Tasks: Only Architect should be ready
    let initial_ready = coordinator.get_ready_tasks();
    assert_eq!(initial_ready.len(), 1);
    assert_eq!(initial_ready[0].id, "task-arch-01");

    // 5. Allocate worktree for Architect and simulate completion
    let arch_worktree = coordinator.allocate_worktree("architect", "task-arch-01");
    assert!(arch_worktree.exists());
    assert!(arch_worktree
        .to_string_lossy()
        .contains("agent-architect-task-arch-01"));

    coordinator
        .mark_running("task-arch-01", arch_worktree)
        .unwrap();
    coordinator
        .mark_completed("task-arch-01", vec!["spec.md".into()])
        .unwrap();

    // 6. Now both Systems and Backend should become Ready
    let next_ready = coordinator.get_ready_tasks();
    assert_eq!(next_ready.len(), 2);
    let ready_ids: Vec<String> = next_ready.into_iter().map(|t| t.id).collect();
    assert!(ready_ids.contains(&"task-sys-02".into()));
    assert!(ready_ids.contains(&"task-backend-03".into()));

    // 7. Complete Systems task
    let sys_worktree = coordinator.allocate_worktree("systems", "task-sys-02");
    coordinator
        .mark_running("task-sys-02", sys_worktree)
        .unwrap();
    coordinator
        .mark_completed("task-sys-02", vec!["storage.rs".into()])
        .unwrap();

    // QA is still blocked because Backend is not yet completed
    let after_sys = coordinator.get_ready_tasks();
    assert_eq!(after_sys.len(), 1);
    assert_eq!(after_sys[0].id, "task-backend-03");

    // 8. Complete Backend task
    let backend_worktree = coordinator.allocate_worktree("backend", "task-backend-03");
    coordinator
        .mark_running("task-backend-03", backend_worktree)
        .unwrap();
    coordinator
        .mark_completed("task-backend-03", vec!["rpc.rs".into()])
        .unwrap();

    // 9. Now QA is finally unlocked
    let qa_ready = coordinator.get_ready_tasks();
    assert_eq!(qa_ready.len(), 1);
    assert_eq!(qa_ready[0].id, "task-qa-04");

    // Clean up temp directory
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_swarm_cycle_detection_prevents_deadlocks() {
    let temp_root =
        std::env::temp_dir().join(format!("aether_swarm_cycle_{}", uuid::Uuid::new_v4()));
    let coordinator = SwarmCoordinator::new(&temp_root);

    // Node A depends on B
    let node_a = SwarmTaskNode::new("A", "Task A", "Desc", "dev", vec!["B".into()]);
    // Node B depends on C
    let node_b = SwarmTaskNode::new("B", "Task B", "Desc", "dev", vec!["C".into()]);
    // Node C depends on A (creates circular dependency: A -> B -> C -> A)
    let node_c = SwarmTaskNode::new("C", "Task C", "Desc", "dev", vec!["A".into()]);

    coordinator.add_task(node_a).unwrap();
    coordinator.add_task(node_b).unwrap();
    let err = coordinator.add_task(node_c);

    assert!(err.is_err(), "Adding cyclic dependency must return error");
    assert!(err.unwrap_err().to_string().contains("cyclic dependency"));

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_three_way_merge_clean_resolution() {
    let base = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\nfn mul(a: i32, b: i32) -> i32 { a * b }";
    // Ours modified add (first line)
    let ours = "fn add(a: i32, b: i32) -> i32 { /* Workspace optimization */ a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\nfn mul(a: i32, b: i32) -> i32 { a * b }";
    // Theirs (Agent Swarm) modified mul (third line)
    let theirs = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\nfn mul(a: i32, b: i32) -> i32 { /* Agent implementation */ a * b }";

    let report = ThreeWayMergeEngine::merge(base, ours, theirs);
    assert!(report.is_clean);
    assert_eq!(report.conflicts.len(), 0);

    let merged = report.merged_content.unwrap();
    assert!(merged.contains("/* Workspace optimization */"));
    assert!(merged.contains("/* Agent implementation */"));
}

#[test]
fn test_three_way_merge_conflict_detection_and_resolution_strategies() {
    let base = "let port = 8080;\n";
    let ours = "let port = 3000;\n";
    let theirs = "let port = 9000;\n";

    let report = ThreeWayMergeEngine::merge(base, ours, theirs);
    assert!(!report.is_clean);
    assert_eq!(report.conflicts.len(), 1);

    // Verify conflict visual markers
    let preview = report.conflict_preview.unwrap();
    assert!(preview.contains("<<<<<<< OURS (Workspace)"));
    assert!(preview.contains("let port = 3000;"));
    assert!(preview.contains("||||||| BASE (Original)"));
    assert!(preview.contains("let port = 8080;"));
    assert!(preview.contains("======="));
    assert!(preview.contains("let port = 9000;"));
    assert!(preview.contains(">>>>>>> THEIRS (Agent Swarm Worktree)"));

    // 1. Resolve by accepting Ours
    let res_ours = ThreeWayMergeEngine::apply_resolutions(
        base,
        ours,
        theirs,
        &[MergeResolutionAction::AcceptOurs],
    );
    assert_eq!(res_ours.trim(), "let port = 3000;");

    // 2. Resolve by accepting Theirs
    let res_theirs = ThreeWayMergeEngine::apply_resolutions(
        base,
        ours,
        theirs,
        &[MergeResolutionAction::AcceptTheirs],
    );
    assert_eq!(res_theirs.trim(), "let port = 9000;");

    // 3. Resolve by accepting Both
    let res_both = ThreeWayMergeEngine::apply_resolutions(
        base,
        ours,
        theirs,
        &[MergeResolutionAction::AcceptBoth],
    );
    assert!(res_both.contains("let port = 3000;"));
    assert!(res_both.contains("let port = 9000;"));
}

#[test]
fn test_inter_agent_message_bus() {
    let temp_root = std::env::temp_dir().join(format!("aether_msg_test_{}", uuid::Uuid::new_v4()));
    let coordinator = SwarmCoordinator::new(&temp_root);

    let msg = AgentMessage::new(
        "systems",
        "backend",
        "task-sys-02",
        AgentMessageType::ProposeDiff,
        "Storage API signature ready",
    );
    coordinator.send_message(msg);

    let msgs_for_backend = coordinator.get_messages_for_persona("backend");
    assert_eq!(msgs_for_backend.len(), 1);
    assert_eq!(msgs_for_backend[0].sender_persona, "systems");
    assert_eq!(msgs_for_backend[0].payload, "Storage API signature ready");

    let _ = std::fs::remove_dir_all(&temp_root);
}
