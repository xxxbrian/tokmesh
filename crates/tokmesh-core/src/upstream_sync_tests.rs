use super::*;
use std::path::{Path, PathBuf};

struct CacheEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl CacheEnv {
    fn capture(keys: &[&'static str]) -> Self {
        Self(
            keys.iter()
                .map(|&key| (key, std::env::var_os(key)))
                .collect(),
        )
    }
    fn set(&mut self, key: &str, value: impl AsRef<std::ffi::OsStr>) {
        std::env::set_var(key, value);
    }
    fn remove(&mut self, key: &str) {
        std::env::remove_var(key);
    }
}

impl Drop for CacheEnv {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn redirect_cache_home(home: &Path) -> CacheEnv {
    let keys = ["HOME", "TOKMESH_CONFIG_DIR"];
    let guard = CacheEnv(
        keys.into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    std::env::set_var("HOME", home);
    std::env::set_var("TOKMESH_CONFIG_DIR", home.join(".config/tokmesh"));
    guard
}

fn client_scan_root(home: &Path, client: ClientId) -> PathBuf {
    PathBuf::from(
        client
            .data()
            .resolve_path_with_env_strategy(&home.to_string_lossy(), false),
    )
}

fn parse_all_messages_with_pricing(
    home: &str,
    clients: &[String],
    pricing: Option<&pricing::PricingService>,
) -> Vec<UnifiedMessage> {
    parse_all_messages_with_pricing_with_env_strategy(
        home,
        clients,
        pricing,
        false,
        &scanner::ScannerSettings::default(),
    )
}

/// Seed `<home>/.openclaw/agents/<agent>/agent/openclaw-agent.sqlite` with
/// one session whose transcript is `events`, returning the store path.
fn seed_openclaw_agent_db(
    home: &std::path::Path,
    agent: &str,
    session_id: &str,
    harness: Option<&str>,
    events: &[String],
) -> std::path::PathBuf {
    use sessions::openclaw::test_fixtures::{create_agent_db, insert_event, insert_session_window};
    let db_path = home
        .join(".openclaw/agents")
        .join(agent)
        .join("agent/openclaw-agent.sqlite");
    let conn = create_agent_db(&db_path);
    insert_session_window(
        &conn,
        session_id,
        Some("anthropic"),
        Some("claude-opus-4-6"),
        harness,
    );
    for (seq, event) in events.iter().enumerate() {
        insert_event(
            &conn,
            session_id,
            seq as i64,
            event,
            1_756_548_000_000 + seq as i64 * 1000,
        );
    }
    drop(conn);
    db_path
}

fn openclaw_assistant_event(id: &str, input: i64, output: i64, timestamp_ms: i64) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","parentId":"u1","message":{{"role":"assistant","content":[{{"type":"text","text":"ok"}}],"provider":"anthropic","model":"claude-opus-4-6","usage":{{"input":{input},"output":{output},"cacheRead":0,"cacheWrite":0,"totalTokens":{total},"cost":{{"total":0}}}},"stopReason":"stop","timestamp":{timestamp_ms}}}}}"#,
        total = input + output
    )
}

fn openclaw_dedup_keys(messages: &[UnifiedMessage]) -> Vec<String> {
    let mut keys: Vec<String> = messages
        .iter()
        .map(|message| {
            message
                .dedup_key
                .clone()
                .expect("openclaw messages carry keys")
        })
        .collect();
    keys.sort();
    keys
}

#[test]
#[serial_test::serial]
fn test_parse_all_messages_counts_openclaw_sqlite_only_usage_across_agents() {
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    // No JSONL transcript anywhere: only the per-agent SQLite stores.
    let header = sessions::openclaw::test_fixtures::header_event("sess-main");
    seed_openclaw_agent_db(
        home,
        "main",
        "sess-main",
        Some("openclaw"),
        &[
            header,
            sessions::openclaw::test_fixtures::user_event("u1", "hi"),
            openclaw_assistant_event("a1", 100, 50, 1_756_548_001_000),
        ],
    );
    seed_openclaw_agent_db(
        home,
        "work",
        "sess-work",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-work"),
            openclaw_assistant_event("b1", 10, 5, 1_756_548_002_000),
        ],
    );

    let messages =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(messages.len(), 2);
    assert!(messages.iter().all(|message| message.client == "openclaw"));
    assert_eq!(
        openclaw_dedup_keys(&messages),
        vec![
            "openclaw:a1:1756548001000:100:50",
            "openclaw:b1:1756548002000:10:5"
        ]
    );
    let input_total: i64 = messages.iter().map(|message| message.tokens.input).sum();
    assert_eq!(input_total, 110);

    // A warm cache replays the same rows.
    let warm =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(openclaw_dedup_keys(&warm), openclaw_dedup_keys(&messages));

    // The uncached lane agrees.
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 2);
    assert_eq!(parsed.messages.len(), 2);
    assert!(parsed.messages.iter().all(|m| m.client == "openclaw"));
}

#[test]
#[serial_test::serial]
fn test_parse_all_messages_openclaw_sqlite_and_legacy_jsonl_do_not_double_count() {
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    // `openclaw doctor --fix` imported this session into SQLite and left
    // the JSONL original in `sessions/`: the same events in both stores.
    let migrated: Vec<String> = vec![
        sessions::openclaw::test_fixtures::header_event("sess-a"),
        r#"{"type":"model_change","id":"m1","provider":"anthropic","modelId":"claude-opus-4-6"}"#
            .to_string(),
        sessions::openclaw::test_fixtures::user_event("u1", "hi"),
        openclaw_assistant_event("a1", 100, 50, 1_756_548_001_000),
        openclaw_assistant_event("a2", 20, 10, 1_756_548_002_000),
    ];
    let sessions_dir = home.join(".openclaw/agents/main/sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(sessions_dir.join("sess-a.jsonl"), migrated.join("\n")).unwrap();
    // A legacy session that was never migrated, and a reset archive
    // OpenClaw published from SQLite after the migration.
    std::fs::write(
        sessions_dir.join("sess-old.jsonl"),
        [
            sessions::openclaw::test_fixtures::header_event("sess-old"),
            openclaw_assistant_event("z1", 7, 3, 1_756_540_000_000),
        ]
        .join("\n"),
    )
    .unwrap();
    std::fs::write(
        sessions_dir.join("sess-reset.jsonl.reset.2026-08-30T10-00-00.000Z"),
        [
            sessions::openclaw::test_fixtures::header_event("sess-reset"),
            openclaw_assistant_event("r1", 5, 5, 1_756_541_000_000),
        ]
        .join("\n"),
    )
    .unwrap();

    seed_openclaw_agent_db(home, "main", "sess-a", None, &migrated);
    // A session that only ever existed in SQLite.
    seed_openclaw_agent_db(
        home,
        "work",
        "sess-new",
        None,
        &[
            sessions::openclaw::test_fixtures::header_event("sess-new"),
            openclaw_assistant_event("n1", 1, 1, 1_756_548_003_000),
        ],
    );

    let expected_keys = vec![
        "openclaw:a1:1756548001000:100:50",
        "openclaw:a2:1756548002000:20:10",
        "openclaw:n1:1756548003000:1:1",
        "openclaw:r1:1756541000000:5:5",
        "openclaw:z1:1756540000000:7:3",
    ];

    let cold =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(openclaw_dedup_keys(&cold), expected_keys);
    let input_total: i64 = cold.iter().map(|message| message.tokens.input).sum();
    assert_eq!(input_total, 100 + 20 + 1 + 7 + 5);

    // Cached entries keep their keys, so a warm scan collapses the same way.
    let warm =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(openclaw_dedup_keys(&warm), expected_keys);

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 5);
    assert_eq!(parsed.messages.len(), 5);
}

#[test]
#[serial_test::serial]
fn test_openclaw_sqlite_source_cache_invalidates_on_wal_only_commit() {
    use sessions::openclaw::test_fixtures::{
        create_agent_db, header_event, insert_event, insert_session_window,
    };
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let db_path = home.join(".openclaw/agents/main/agent/openclaw-agent.sqlite");
    let conn = create_agent_db(&db_path);
    // Keep every commit in the WAL, the way a running gateway between
    // checkpoints does, so the main file never changes.
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    insert_session_window(
        &conn,
        "sess-a",
        Some("anthropic"),
        Some("claude-opus-4-6"),
        None,
    );
    insert_event(
        &conn,
        "sess-a",
        0,
        &header_event("sess-a"),
        1_756_548_000_000,
    );
    insert_event(
        &conn,
        "sess-a",
        1,
        &openclaw_assistant_event("a1", 100, 50, 1_756_548_001_000),
        1_756_548_001_000,
    );

    let first =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(
        openclaw_dedup_keys(&first),
        vec!["openclaw:a1:1756548001000:100:50"]
    );
    let main_len_before = std::fs::metadata(&db_path).unwrap().len();

    insert_event(
        &conn,
        "sess-a",
        2,
        &openclaw_assistant_event("a2", 20, 10, 1_756_548_002_000),
        1_756_548_002_000,
    );
    let wal_path = db_path.with_file_name("openclaw-agent.sqlite-wal");
    assert!(wal_path.exists(), "the commit must have gone to the WAL");
    assert_eq!(
        std::fs::metadata(&db_path).unwrap().len(),
        main_len_before,
        "the main database file must be untouched, so only the WAL can reveal the row"
    );

    let refreshed =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(
        openclaw_dedup_keys(&refreshed),
        vec![
            "openclaw:a1:1756548001000:100:50",
            "openclaw:a2:1756548002000:20:10"
        ]
    );
    drop(conn);
}

/// The assistant row OpenClaw mirrors from a Codex app-server turn: only
/// the last model response's usage (400/800/300 input/cached/output),
/// keyed by the Codex thread and turn.
fn openclaw_codex_mirror_event(id: &str, thread: &str, turn: &str, timestamp_ms: i64) -> String {
    openclaw_codex_mirror_event_with_usage(id, thread, turn, timestamp_ms, (400, 800, 300))
}

/// [`openclaw_codex_mirror_event`] with the mirrored usage spelled out as
/// `(input, cacheRead, output)`.
fn openclaw_codex_mirror_event_with_usage(
    id: &str,
    thread: &str,
    turn: &str,
    timestamp_ms: i64,
    (input, cache_read, output): (i64, i64, i64),
) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","parentId":"u1","message":{{"role":"assistant","content":[{{"type":"text","text":"done"}}],"api":"openai-chatgpt-responses","provider":"openai","model":"gpt-5.2-codex","usage":{{"input":{input},"output":{output},"cacheRead":{cache_read},"cacheWrite":0,"totalTokens":{total},"cost":{{"total":0}}}},"idempotencyKey":"codex-app-server:{thread}:{turn}:assistant","__openclaw":{{"mirrorOrigin":"codex-app-server","mirrorIdentity":"{turn}:assistant"}},"stopReason":"stop","timestamp":{timestamp_ms}}}}}"#,
        total = input + cache_read + output
    )
}

/// One model response of a Codex turn as `last_token_usage` reports it:
/// `(input_tokens, cached_input_tokens, output_tokens)`, input inclusive
/// of cached.
type CodexResponse = (i64, i64, i64);

/// A Codex rollout of the given turns, each announced by `task_started`
/// and `turn_context` carrying its `turn_id` the way current Codex writes
/// them, then one token_count per response with cumulative totals.
fn openclaw_codex_rollout_with_turns(
    thread: &str,
    originator: &str,
    turns: &[(&str, &[CodexResponse])],
) -> String {
    openclaw_codex_rollout_lines(thread, originator, turns, 1).join("\n") + "\n"
}

/// The lines [`openclaw_codex_rollout_with_turns`] appends for `turns`
/// alone, timestamped from minute `minute`, so a test can extend a
/// rollout the way Codex does when a later turn runs.
fn openclaw_codex_rollout_turn_lines(
    turns: &[(&str, &[CodexResponse])],
    minute: usize,
    totals: &mut CodexResponse,
) -> Vec<String> {
    let mut lines = Vec::new();
    for (turn_index, (turn_id, responses)) in turns.iter().enumerate() {
        let minute = minute + turn_index;
        lines.push(format!(
                r#"{{"timestamp":"2026-08-30T10:{minute:02}:00Z","type":"event_msg","payload":{{"type":"task_started","turn_id":"{turn_id}","started_at":{}}}}}"#,
                1_756_548_000 + minute as i64 * 60
            ));
        lines.push(format!(
                r#"{{"timestamp":"2026-08-30T10:{minute:02}:00Z","type":"turn_context","payload":{{"turn_id":"{turn_id}","model":"gpt-5.2-codex"}}}}"#
            ));
        for (second, (input, cached, output)) in responses.iter().enumerate() {
            totals.0 += input;
            totals.1 += cached;
            totals.2 += output;
            lines.push(format!(
                    r#"{{"timestamp":"2026-08-30T10:{minute:02}:{:02}Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{},"cached_input_tokens":{},"output_tokens":{}}},"last_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"output_tokens":{output}}}}}}}}}"#,
                    second + 1,
                    totals.0,
                    totals.1,
                    totals.2
                ));
        }
    }
    lines
}

fn openclaw_codex_rollout_lines(
    thread: &str,
    originator: &str,
    turns: &[(&str, &[CodexResponse])],
    minute: usize,
) -> Vec<String> {
    let mut lines = vec![format!(
        r#"{{"timestamp":"2026-08-30T10:00:00Z","type":"session_meta","payload":{{"id":"{thread}","originator":"{originator}","source":"cli","model_provider":"openai","cwd":"/home/alice/.openclaw/workspace"}}}}"#
    )];
    let mut totals = (0, 0, 0);
    lines.extend(openclaw_codex_rollout_turn_lines(
        turns,
        minute,
        &mut totals,
    ));
    lines
}

/// The two responses of [`openclaw_codex_rollout`] as a turn: 1000/700/50
/// and 1200/800/300 input/cached/output, which the codex parser reports
/// as 300/700/50 and 400/800/300 input/cache_read/output.
const OPENCLAW_CODEX_TURN_1: &[CodexResponse] = &[(1000, 700, 50), (1200, 800, 300)];

/// A Codex rollout of one OpenClaw-driven turn with two model responses
/// (a tool call, then the final answer): 1000/700/50 and 1200/800/300
/// input/cached/output.
fn openclaw_codex_rollout(thread: &str, originator: &str) -> String {
    format!(
        concat!(
            r#"{{"timestamp":"2026-08-30T10:00:00Z","type":"session_meta","payload":{{"id":"{thread}","originator":"{originator}","source":"cli","model_provider":"openai","cwd":"/home/alice/.openclaw/workspace"}}}}"#,
            "\n",
            r#"{{"timestamp":"2026-08-30T10:00:01Z","type":"turn_context","payload":{{"model":"gpt-5.2-codex"}}}}"#,
            "\n",
            r#"{{"timestamp":"2026-08-30T10:00:02Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":1000,"cached_input_tokens":700,"output_tokens":50}},"last_token_usage":{{"input_tokens":1000,"cached_input_tokens":700,"output_tokens":50}}}}}}}}"#,
            "\n",
            r#"{{"timestamp":"2026-08-30T10:00:05Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":2200,"cached_input_tokens":1500,"output_tokens":350}},"last_token_usage":{{"input_tokens":1200,"cached_input_tokens":800,"output_tokens":300}}}}}}}}"#,
            "\n"
        ),
        thread = thread,
        originator = originator,
    )
}

const OPENCLAW_CODEX_THREAD: &str = "0192f3a4-5b6c-7d8e-9f01-23456789abcd";

fn openclaw_usage_by_client_session(
    messages: &[UnifiedMessage],
) -> Vec<(String, String, i64, i64, i64)> {
    let mut rows: Vec<(String, String, i64, i64, i64)> = messages
        .iter()
        .map(|message| {
            (
                message.client.clone(),
                message.session_id.clone(),
                message.tokens.input,
                message.tokens.cache_read,
                message.tokens.output,
            )
        })
        .collect();
    rows.sort();
    rows
}

#[test]
#[serial_test::serial]
fn test_openclaw_codex_home_rollout_replaces_the_transcript_mirror_rows() {
    // Default OpenClaw: Codex app-server runs with CODEX_HOME inside the
    // agent dir, so the rollout sits under `agent/codex-home/sessions`.
    // The transcript mirrors only the final response (400/800/300); the
    // rollout has both responses. The rollout wins, under the OpenClaw
    // session, and the mirror row goes; OpenClaw's own runtime turns in
    // the same session are untouched.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let rollout_dir = home.join(".openclaw/agents/main/agent/codex-home/sessions/2026/08/30");
    std::fs::create_dir_all(&rollout_dir).unwrap();
    std::fs::write(
        rollout_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout(OPENCLAW_CODEX_THREAD, "openclaw"),
    )
    .unwrap();
    // Other files Codex keeps in its home are not usage in any format.
    std::fs::write(
        home.join(".openclaw/agents/main/agent/codex-home/history.jsonl"),
        "{\"session_id\":\"x\",\"ts\":1,\"text\":\"ls\"}\n",
    )
    .unwrap();

    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            sessions::openclaw::test_fixtures::user_event("u1", "hi"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
            // A turn this session ran on OpenClaw's own runtime afterwards.
            openclaw_assistant_event("a2", 30, 10, 1_756_548_009_000),
        ],
    );

    let expected = vec![
        ("openclaw".to_string(), "sess-codex".to_string(), 30, 0, 10),
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            300,
            700,
            50,
        ),
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            400,
            800,
            300,
        ),
    ];

    let cold = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["openclaw".to_string(), "codex".to_string()],
        None,
    );
    assert_eq!(openclaw_usage_by_client_session(&cold), expected);
    assert!(cold.iter().all(|message| message.client == "openclaw"));

    let warm = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["openclaw".to_string(), "codex".to_string()],
        None,
    );
    assert_eq!(openclaw_usage_by_client_session(&warm), expected);

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["openclaw".to_string(), "codex".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 3);
    assert_eq!(parsed.counts.get(ClientId::Codex), 0);
    let mut uncached: Vec<(String, i64, i64)> = parsed
        .messages
        .iter()
        .map(|m| (m.session_id.clone(), m.input, m.output))
        .collect();
    uncached.sort();
    assert_eq!(
        uncached,
        vec![
            ("sess-codex".to_string(), 30, 10),
            ("sess-codex".to_string(), 300, 50),
            ("sess-codex".to_string(), 400, 300),
        ]
    );
}

#[test]
#[serial_test::serial]
fn test_openclaw_codex_mirror_rows_stay_when_no_rollout_was_read() {
    // No rollout anywhere for this thread (ephemeral, pruned, or on a
    // paired node): the mirror's last-response usage is the only record
    // and must remain.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
        ],
    );

    let messages = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["openclaw".to_string(), "codex".to_string()],
        None,
    );
    assert_eq!(
        openclaw_usage_by_client_session(&messages),
        vec![(
            "openclaw".to_string(),
            "sess-codex".to_string(),
            400,
            800,
            300
        )]
    );
}

#[test]
#[serial_test::serial]
fn test_openclaw_originated_rollouts_in_the_user_codex_home_are_openclaw_usage() {
    // `appServer.homeScope: "user"` (or a supervision branch) makes
    // OpenClaw create its Codex threads in the user's own `~/.codex`,
    // where the codex scanner reads them. Their originator names
    // OpenClaw, so the codex lane hands them to the openclaw lane, which
    // replaces the transcript's mirror row with them. The user's own
    // Codex threads keep counting under codex.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let codex_dir = home.join(".codex/sessions/2026/08/30");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout(OPENCLAW_CODEX_THREAD, "openclaw"),
    )
    .unwrap();
    std::fs::write(
        codex_dir.join("rollout-2026-08-30T11-00-00-11111111-2222-3333-4444-555555555555.jsonl"),
        openclaw_codex_rollout("11111111-2222-3333-4444-555555555555", "codex-tui"),
    )
    .unwrap();

    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
        ],
    );

    let both = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["codex".to_string(), "openclaw".to_string()],
        None,
    );
    assert_eq!(
        openclaw_usage_by_client_session(&both),
        vec![
            (
                "codex".to_string(),
                "rollout-2026-08-30T11-00-00-11111111-2222-3333-4444-555555555555".to_string(),
                300,
                700,
                50
            ),
            (
                "codex".to_string(),
                "rollout-2026-08-30T11-00-00-11111111-2222-3333-4444-555555555555".to_string(),
                400,
                800,
                300
            ),
            (
                "openclaw".to_string(),
                "sess-codex".to_string(),
                300,
                700,
                50
            ),
            (
                "openclaw".to_string(),
                "sess-codex".to_string(),
                400,
                800,
                300
            ),
        ]
    );

    // A warm scan replays the same partition from the cache.
    let warm = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["codex".to_string(), "openclaw".to_string()],
        None,
    );
    assert_eq!(
        openclaw_usage_by_client_session(&warm),
        openclaw_usage_by_client_session(&both)
    );

    // Asking for codex alone never shows OpenClaw's turns under codex.
    let codex_only =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["codex".to_string()], None);
    assert_eq!(codex_only.len(), 2);
    assert!(codex_only.iter().all(|message| message.client == "codex"));

    // Asking for openclaw alone still finds the rollout in the user's
    // Codex home (the scanner walks it as a lookup) and never shows the
    // user's own Codex thread.
    let openclaw_only =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(
        openclaw_usage_by_client_session(&openclaw_only),
        vec![
            (
                "openclaw".to_string(),
                "sess-codex".to_string(),
                300,
                700,
                50
            ),
            (
                "openclaw".to_string(),
                "sess-codex".to_string(),
                400,
                800,
                300
            ),
        ]
    );
    let openclaw_only_parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(openclaw_only_parsed.counts.get(ClientId::Codex), 0);
    assert_eq!(openclaw_only_parsed.counts.get(ClientId::OpenClaw), 2);
    assert!(openclaw_only_parsed
        .messages
        .iter()
        .all(|m| m.client == "openclaw"));

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["codex".to_string(), "openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::Codex), 2);
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 2);
    assert!(parsed
        .messages
        .iter()
        .filter(|m| m.client == "openclaw")
        .all(|m| m.session_id == "sess-codex"));

    // Codex alone: the OpenClaw-owned rollout is neither returned nor
    // counted, under either client.
    let codex_only_parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["codex".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(codex_only_parsed.counts.get(ClientId::Codex), 2);
    assert_eq!(codex_only_parsed.counts.get(ClientId::OpenClaw), 0);
    assert_eq!(codex_only_parsed.messages.len(), 2);
    assert!(codex_only_parsed
        .messages
        .iter()
        .all(|m| m.client == "codex"));
}

#[test]
#[serial_test::serial]
fn test_openclaw_mirror_rows_yield_to_a_thread_the_codex_lane_counted() {
    // Supervision lets OpenClaw resume a thread the user created in their
    // own Codex home. The rollout keeps its original originator, so the
    // codex lane counts it; OpenClaw's mirror of the turns it drove must
    // then go, or the turns count under both clients.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let codex_dir = home.join(".codex/sessions/2026/08/30");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout(OPENCLAW_CODEX_THREAD, "Codex Desktop"),
    )
    .unwrap();

    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
            openclaw_assistant_event("a2", 30, 10, 1_756_548_009_000),
        ],
    );

    let expected = vec![
        (
            "codex".to_string(),
            format!("rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}"),
            300,
            700,
            50,
        ),
        (
            "codex".to_string(),
            format!("rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}"),
            400,
            800,
            300,
        ),
        ("openclaw".to_string(), "sess-codex".to_string(), 30, 0, 10),
    ];
    let cold = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["codex".to_string(), "openclaw".to_string()],
        None,
    );
    assert_eq!(openclaw_usage_by_client_session(&cold), expected);
    let warm = parse_all_messages_with_pricing(
        home.to_str().unwrap(),
        &["codex".to_string(), "openclaw".to_string()],
        None,
    );
    assert_eq!(openclaw_usage_by_client_session(&warm), expected);

    // With codex not requested, nothing is counted under codex, so the
    // mirror is the only record of those turns and stays.
    let openclaw_only =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(
        openclaw_usage_by_client_session(&openclaw_only),
        vec![
            ("openclaw".to_string(), "sess-codex".to_string(), 30, 0, 10),
            (
                "openclaw".to_string(),
                "sess-codex".to_string(),
                400,
                800,
                300
            ),
        ]
    );

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["codex".to_string(), "openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::Codex), 2);
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 1);
}

/// Carried over from #1291, which #1285 made necessary and this branch
/// makes redundant: a plain transcript and its zstd archive in one
/// sessions directory count each event once through the cached cold
/// scan, the warm scan, and the direct aggregate, while archive-only
/// history survives and events with no id stay distinct rather than
/// collapsing on usage alone.
#[test]
#[serial_test::serial]
fn test_openclaw_plain_and_compressed_archives_dedup_events_across_aggregate_paths() {
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let sessions_dir = client_scan_root(source_home.path(), ClientId::OpenClaw)
        .join("main")
        .join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let model_change =
        r#"{"type":"model_change","provider":"anthropic","modelId":"claude-sonnet-4-6"}"#;
    let shared = r#"{"type":"message","id":"shared-event","message":{"role":"assistant","content":[{"type":"text","text":"shared"}],"usage":{"input":100,"output":10},"timestamp":1788566869000}}"#;
    let distinct = r#"{"type":"message","id":"distinct-event","message":{"role":"assistant","content":[{"type":"text","text":"distinct"}],"usage":{"input":100,"output":10},"timestamp":1788566869000}}"#;
    let plain_idless = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"plain idless"}],"usage":{"input":7,"output":1},"timestamp":1788566870000}}"#;
    let archive_only = r#"{"type":"message","id":"archive-only","message":{"role":"assistant","content":[{"type":"text","text":"older history"}],"usage":{"input":40,"output":4},"timestamp":1788566800000}}"#;
    let archive_idless = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"archive idless"}],"usage":{"input":7,"output":1},"timestamp":1788566870000}}"#;

    std::fs::write(
        sessions_dir.join("session.jsonl"),
        [model_change, shared, distinct, plain_idless].join("\n"),
    )
    .unwrap();
    let archive = [model_change, archive_only, shared, archive_idless].join("\n");
    std::fs::write(
        sessions_dir.join("session.jsonl.deleted.2026-09-05T00-00-00.000Z.zst"),
        zstd::encode_all(archive.as_bytes(), 0).unwrap(),
    )
    .unwrap();

    let clients = vec!["openclaw".to_string()];
    let home = source_home.path().to_str().unwrap();
    let cold = parse_all_messages_with_pricing(home, &clients, None);
    let warm = parse_all_messages_with_pricing(home, &clients, None);
    let direct = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_string()),
        use_env_roots: false,
        clients: Some(clients),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();

    for (path, messages) in [("cold", &cold), ("warm", &warm)] {
        assert_eq!(messages.len(), 5, "{path}");
        assert_eq!(
            messages
                .iter()
                .map(|message| message.tokens.input)
                .sum::<i64>(),
            254,
            "{path}",
        );
    }
    assert_eq!(warm, cold);
    assert_eq!(
        cold.iter()
            .filter(|message| {
                message.dedup_key.as_deref() == Some("openclaw:shared-event:1788566869000:100:10")
            })
            .count(),
        1,
    );
    assert_eq!(
        cold.iter()
            .filter(|message| message.dedup_key.is_none())
            .count(),
        2,
        "events without ids must stay distinct rather than collapsing on usage",
    );
    assert_eq!(direct.counts.get(ClientId::OpenClaw), 5);
    assert_eq!(direct.messages.len(), 5);
    assert_eq!(
        direct
            .messages
            .iter()
            .map(|message| message.input)
            .sum::<i64>(),
        254,
    );
}
#[test]
#[serial_test::serial]
fn test_openclaw_fork_copies_count_once_across_stores_and_sessions() {
    // `/fork` copies the visible transcript into a new session with the
    // same event ids, timestamps and usage, and the legacy JSONL of the
    // original may still be on disk beside the SQLite rows. One spend,
    // three copies, one count.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let original = openclaw_assistant_event("a1", 100, 50, 1_756_548_001_000);
    let sessions_dir = home.join(".openclaw/agents/main/sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(
        sessions_dir.join("sess-a.jsonl"),
        [
            sessions::openclaw::test_fixtures::header_event("sess-a"),
            original.clone(),
        ]
        .join("\n"),
    )
    .unwrap();
    {
        use sessions::openclaw::test_fixtures::{
            create_agent_db, header_event, insert_event, insert_session_window,
        };
        let db_path = home.join(".openclaw/agents/main/agent/openclaw-agent.sqlite");
        let conn = create_agent_db(&db_path);
        for session in ["sess-a", "sess-fork"] {
            insert_session_window(
                &conn,
                session,
                Some("anthropic"),
                Some("claude-opus-4-6"),
                None,
            );
            insert_event(&conn, session, 0, &header_event(session), 1);
            insert_event(&conn, session, 1, &original, 1_756_548_001_000);
        }
        // The fork's own new turn.
        insert_event(
            &conn,
            "sess-fork",
            2,
            &openclaw_assistant_event("f1", 7, 3, 1_756_548_500_000),
            1_756_548_500_000,
        );
    }

    let messages =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(
        openclaw_dedup_keys(&messages),
        vec![
            "openclaw:a1:1756548001000:100:50",
            "openclaw:f1:1756548500000:7:3"
        ]
    );

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(vec!["openclaw".to_string()]),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 2);
}

#[test]
#[serial_test::serial]
fn test_openclaw_rollout_stands_in_only_for_the_turns_it_holds() {
    // The rollout holds turn 1 (two responses) and nothing of turn 2: it
    // was cut short, or turn 2 ran after it was read. Its record replaces
    // the transcript's mirror row for turn 1; the mirror row for turn 2 is
    // the only record of that turn and stays. Once turn 2 is appended to
    // the rollout, its mirror row yields too. Both lanes, and the cached
    // lane through a warm scan and an incremental resume.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let rollout_dir = home.join(".openclaw/agents/main/agent/codex-home/sessions/2026/08/30");
    std::fs::create_dir_all(&rollout_dir).unwrap();
    let rollout_path = rollout_dir.join(format!(
        "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
    ));
    std::fs::write(
        &rollout_path,
        openclaw_codex_rollout_with_turns(
            OPENCLAW_CODEX_THREAD,
            "openclaw",
            &[("turn-1", OPENCLAW_CODEX_TURN_1)],
        ),
    )
    .unwrap();
    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
            openclaw_codex_mirror_event_with_usage(
                "m2",
                OPENCLAW_CODEX_THREAD,
                "turn-2",
                1_756_548_065_000,
                (60, 20, 12),
            ),
        ],
    );
    let clients = ["openclaw".to_string(), "codex".to_string()];
    let row = |input, cache_read, output| {
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            input,
            cache_read,
            output,
        )
    };

    let expected = vec![row(60, 20, 12), row(300, 700, 50), row(400, 800, 300)];
    let cold = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&cold), expected);
    let warm = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&warm), expected);
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(clients.to_vec()),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 3);
    assert_eq!(parsed.counts.get(ClientId::Codex), 0);

    // Turn 2 reaches the rollout: one response of 500/100/40.
    let mut totals = (2200, 1500, 350);
    let appended =
        openclaw_codex_rollout_turn_lines(&[("turn-2", &[(500, 100, 40)])], 1, &mut totals)
            .join("\n")
            + "\n";
    {
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&rollout_path)
            .unwrap()
            .write_all(appended.as_bytes())
            .unwrap();
    }
    let expected = vec![row(300, 700, 50), row(400, 100, 40), row(400, 800, 300)];
    let resumed = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&resumed), expected);
    let warm = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&warm), expected);
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(clients.to_vec()),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 3);
}

#[test]
#[serial_test::serial]
fn test_openclaw_rollout_without_turn_ids_stands_in_for_its_whole_thread() {
    // A rollout written before Codex stamped `turn_id` on its turns
    // cannot be matched per turn. It stands in for the whole thread,
    // which is what the lanes did before coverage was per turn and all
    // such a rollout allows.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let rollout_dir = home.join(".openclaw/agents/main/agent/codex-home/sessions/2026/08/30");
    std::fs::create_dir_all(&rollout_dir).unwrap();
    std::fs::write(
        rollout_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout(OPENCLAW_CODEX_THREAD, "openclaw"),
    )
    .unwrap();
    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
            openclaw_codex_mirror_event_with_usage(
                "m2",
                OPENCLAW_CODEX_THREAD,
                "turn-2",
                1_756_548_065_000,
                (60, 20, 12),
            ),
        ],
    );
    let clients = ["openclaw".to_string()];
    let expected = vec![
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            300,
            700,
            50,
        ),
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            400,
            800,
            300,
        ),
    ];
    let cold = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&cold), expected);
    let warm = parse_all_messages_with_pricing(home.to_str().unwrap(), &clients, None);
    assert_eq!(openclaw_usage_by_client_session(&warm), expected);
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(clients.to_vec()),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 2);
}

#[test]
#[serial_test::serial]
fn test_openclaw_codex_home_rollout_counts_once_when_codex_home_points_at_it() {
    // `CODEX_HOME` aimed at an agent's codex-home makes the Codex roots
    // overlap the OpenClaw agents tree. The rollout's originator does not
    // name OpenClaw (an older OpenClaw), so by metadata alone the codex
    // lane would count it under codex while the openclaw lane counts it
    // by location, each behind its own dedup set. The scanner decides
    // ownership once: the openclaw lane alone reads it.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let codex_home = home.join(".openclaw/agents/main/agent/codex-home");
    let rollout_dir = codex_home.join("sessions/2026/08/30");
    std::fs::create_dir_all(&rollout_dir).unwrap();
    std::fs::write(
        rollout_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout_with_turns(
            OPENCLAW_CODEX_THREAD,
            "codex-tui",
            &[("turn-1", OPENCLAW_CODEX_TURN_1)],
        ),
    )
    .unwrap();
    seed_openclaw_agent_db(
        home,
        "main",
        "sess-codex",
        Some("codex"),
        &[
            sessions::openclaw::test_fixtures::header_event("sess-codex"),
            openclaw_codex_mirror_event("m1", OPENCLAW_CODEX_THREAD, "turn-1", 1_756_548_005_000),
        ],
    );
    let mut env = CacheEnv::capture(&["CODEX_HOME", "TOKMESH_EXTRA_DIRS", "TOKMESH_HEADLESS_DIR"]);
    env.set("CODEX_HOME", &codex_home);
    env.remove("TOKMESH_EXTRA_DIRS");
    env.remove("TOKMESH_HEADLESS_DIR");

    let both = ["codex".to_string(), "openclaw".to_string()];
    let expected = vec![
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            300,
            700,
            50,
        ),
        (
            "openclaw".to_string(),
            "sess-codex".to_string(),
            400,
            800,
            300,
        ),
    ];
    for _ in 0..2 {
        let messages = parse_all_messages_with_pricing_with_env_strategy(
            home.to_str().unwrap(),
            &both,
            None,
            true,
            &scanner::ScannerSettings::default(),
        );
        assert_eq!(openclaw_usage_by_client_session(&messages), expected);
    }
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: true,
        clients: Some(both.to_vec()),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::Codex), 0);
    assert_eq!(parsed.counts.get(ClientId::OpenClaw), 2);

    // Without openclaw in the request the agents tree is never walked,
    // and the rollout is what its metadata says: Codex usage in the
    // directory the user pointed Codex at.
    let codex_only = parse_all_messages_with_pricing_with_env_strategy(
        home.to_str().unwrap(),
        &["codex".to_string()],
        None,
        true,
        &scanner::ScannerSettings::default(),
    );
    assert_eq!(codex_only.len(), 2);
    assert!(codex_only.iter().all(|message| message.client == "codex"));
}

#[test]
#[serial_test::serial]
fn test_synthetic_request_keeps_codex_usage_a_synthetic_gateway_served() {
    // `--client synthetic` keeps whatever any client routed through a
    // synthetic gateway; the flush filter is where that is decided, and
    // Codex usage has to reach it whether or not codex was named too.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();

    let codex_dir = home.join(".codex/sessions/2026/08/30");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join(format!(
            "rollout-2026-08-30T10-00-00-{OPENCLAW_CODEX_THREAD}.jsonl"
        )),
        openclaw_codex_rollout_with_turns(
            OPENCLAW_CODEX_THREAD,
            "codex-tui",
            &[("turn-1", OPENCLAW_CODEX_TURN_1)],
        )
        .replace(
            r#""model_provider":"openai""#,
            r#""model_provider":"synthetic""#,
        ),
    )
    .unwrap();
    std::fs::write(
        codex_dir.join("rollout-2026-08-30T11-00-00-11111111-2222-3333-4444-555555555555.jsonl"),
        openclaw_codex_rollout_with_turns(
            "11111111-2222-3333-4444-555555555555",
            "codex-tui",
            &[("turn-1", &[(10, 0, 5)])],
        ),
    )
    .unwrap();

    let synthetic = ["synthetic".to_string()];
    let messages = parse_all_messages_with_pricing(home.to_str().unwrap(), &synthetic, None);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(messages.iter().all(|message| message.client == "codex"));
    assert!(messages.iter().all(|message| message.tokens.input >= 300));

    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: Some(home.to_str().unwrap().to_string()),
        use_env_roots: false,
        clients: Some(synthetic.to_vec()),
        since: None,
        until: None,
        year: None,
        scanner_settings: scanner::ScannerSettings::default(),
    })
    .unwrap();
    assert_eq!(parsed.counts.get(ClientId::Codex), 2);
    assert_eq!(parsed.messages.len(), 2);
    assert!(parsed
        .messages
        .iter()
        .all(|message| message.client == "codex"));
}

#[test]
#[serial_test::serial]
fn test_openclaw_store_read_partway_is_reported_but_not_cached() {
    // A store whose read stops partway (the file is cut off under a table
    // spanning many pages) contributes the rows it yielded to this scan,
    // but no cache entry: cached, that prefix would be served as the
    // store on every warm scan until the file happened to change.
    let cache_home = tempfile::TempDir::new().unwrap();
    let source_home = tempfile::TempDir::new().unwrap();
    let _cache_env = redirect_cache_home(cache_home.path());
    let home = source_home.path();
    let db_path = home.join(".openclaw/agents/main/agent/openclaw-agent.sqlite");
    let identity = message_cache::CacheIdentity::for_client(ClientId::OpenClaw);

    let seed = |sessions: i64| {
        use sessions::openclaw::test_fixtures::{
            create_agent_db, header_event, insert_event, insert_session_window,
        };
        let _ = std::fs::remove_file(&db_path);
        let conn = create_agent_db(&db_path);
        for index in 0..sessions {
            let session = format!("sess-{index:04}");
            insert_session_window(
                &conn,
                &session,
                Some("anthropic"),
                Some("claude-opus-4-6"),
                None,
            );
            insert_event(&conn, &session, 0, &header_event(&session), 1);
            insert_event(
                &conn,
                &session,
                1,
                &openclaw_assistant_event(
                    &format!("a{index:04}"),
                    10,
                    5,
                    1_756_548_000_000 + index,
                ),
                1_756_548_000_000 + index,
            );
        }
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);
    };

    seed(400);
    let size = std::fs::metadata(&db_path).unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&db_path)
        .unwrap()
        .set_len(size / 3)
        .unwrap();
    let partial =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert!(partial.len() < 400, "{}", partial.len());
    let cache = message_cache::SourceMessageCache::load();
    assert!(
        cache.get(identity, &db_path).is_none(),
        "a store read partway must not be cached"
    );
    drop(cache);

    // An intact store is.
    seed(4);
    let complete =
        parse_all_messages_with_pricing(home.to_str().unwrap(), &["openclaw".to_string()], None);
    assert_eq!(complete.len(), 4);
    let cache = message_cache::SourceMessageCache::load();
    assert!(cache.get(identity, &db_path).is_some());
}
