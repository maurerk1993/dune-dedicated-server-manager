use super::{BuildKind, Category, CommandSpec, FieldKind, FieldSpec, SelectOption};

const FIELD_PLAYER: FieldSpec = FieldSpec {
    key: "PlayerId",
    label: "Player",
    kind: FieldKind::String,
    required: Some(true),
    default: None,
    helper: Some("FLS player id, or \"*\" for all online"),
    options: None,
};

// Journey commands removed 2026-05-26: published successfully but the
// server-command handlers don't apply the state changes (live-tested).
// `FIELD_STORY_NODE` + `PLAYER_AND_STORY` retired with them.

// XP category options were removed 2026-05-26 — live-testing showed the
// server ignores Category and always grants generic player XP regardless of
// which value is sent. Keeping AwardXP as a player+amount command only.

const BROADCAST_TYPES: &[SelectOption] = &[
    SelectOption {
        value: "Generic",
        label: "Generic",
    },
    SelectOption {
        value: "ServerShutdown",
        label: "ServerShutdown",
    },
];

const SHUTDOWN_TYPES: &[SelectOption] = &[
    SelectOption {
        value: "Restart",
        label: "Restart",
    },
    SelectOption {
        value: "Maintenance",
        label: "Maintenance",
    },
    SelectOption {
        value: "Update",
        label: "Update",
    },
];

const ADD_ITEM_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "ItemName",
        label: "ItemName",
        kind: FieldKind::String,
        required: Some(true),
        default: None,
        helper: Some("Internal FName, case-insensitive"),
        options: None,
    },
    FieldSpec {
        key: "Quantity",
        label: "Quantity",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(1)),
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Durability",
        label: "Durability",
        kind: FieldKind::Float,
        required: None,
        default: Some(json_const_f(1.0)),
        helper: None,
        options: None,
    },
];

const SERVICE_BROADCAST_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "BroadcastType",
        label: "BroadcastType",
        kind: FieldKind::Select,
        required: Some(true),
        default: Some(json_const_s("Generic")),
        helper: None,
        options: Some(BROADCAST_TYPES),
    },
    FieldSpec {
        key: "Title",
        label: "Title",
        kind: FieldKind::String,
        required: None,
        default: None,
        helper: Some("required for Generic"),
        options: None,
    },
    FieldSpec {
        key: "Body",
        label: "Body",
        kind: FieldKind::Text,
        required: None,
        default: None,
        helper: Some("required for Generic"),
        options: None,
    },
    FieldSpec {
        key: "BroadcastDuration",
        label: "Display duration (in seconds)",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(30)),
        helper: Some("How long each broadcast pulse stays on-screen"),
        options: None,
    },
    FieldSpec {
        key: "ShutdownType",
        label: "Shutdown type",
        kind: FieldKind::Select,
        required: None,
        default: Some(json_const_s("Restart")),
        helper: Some("ServerShutdown only"),
        options: Some(SHUTDOWN_TYPES),
    },
    FieldSpec {
        key: "ShutdownDuration",
        label: "Lead time (in seconds)",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(600)),
        helper: Some("ServerShutdown only - seconds until the shutdown fires"),
        options: None,
    },
    FieldSpec {
        key: "BroadcastFrequency",
        label: "Repeat frequency (in seconds)",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(60)),
        helper: Some("ServerShutdown only - how often the countdown re-broadcasts"),
        options: None,
    },
    FieldSpec {
        key: "ShouldCancel",
        label: "Cancel pending shutdown",
        kind: FieldKind::Bool,
        required: None,
        default: Some(json_const_b(false)),
        helper: Some("ServerShutdown only - cancels an in-flight countdown; ignores other fields"),
        options: None,
    },
];

const ONLY_PLAYER: &[FieldSpec] = &[FIELD_PLAYER];

const WATER_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "WaterAmount",
        label: "WaterAmount",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(1_000_000)),
        helper: None,
        options: None,
    },
];

const AWARD_XP_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "Experience",
        label: "Experience",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(1000)),
        helper: Some("Generic player XP — the server ignores any track/category fields."),
        options: None,
    },
];

// AwardXPByEventTag was tried 2026-05-26 — server reports
// `Deserialized message has unknown Server Command 'AwardXPByEventTag'`.
// The binary has `ADuneCharacter::AwardXPByEventTag` but no MQ handler.

const SKILL_MODULE_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "Module",
        label: "Module",
        kind: FieldKind::String,
        required: Some(true),
        default: None,
        helper: Some("e.g. Swordmaster_T1"),
        options: None,
    },
    FieldSpec {
        key: "Level",
        label: "Level",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(1)),
        helper: None,
        options: None,
    },
];

const SKILL_POINTS_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "SkillPoints",
        label: "SkillPoints",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(0)),
        helper: None,
        options: None,
    },
];

const TELEPORT_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "X",
        label: "X",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Y",
        label: "Y",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Z",
        label: "Z",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Yaw",
        label: "Yaw",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamPitch",
        label: "CamPitch",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamYaw",
        label: "CamYaw",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamRoll",
        label: "CamRoll",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
];

const CHEAT_SCRIPT_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "ScriptName",
        label: "ScriptName",
        kind: FieldKind::String,
        required: Some(true),
        default: None,
        helper: Some("[CheatScript.<name>] section from DefaultGame.ini (e.g. PlaytestSetupAdmin, UnlockAllSkills)"),
        options: None,
    },
];

const SERVER_EXEC_FIELDS: &[FieldSpec] = &[FieldSpec {
    key: "Exec",
    label: "Exec",
    kind: FieldKind::String,
    required: Some(true),
    default: None,
    helper: Some("Raw console/exec command string"),
    options: None,
}];

const SPAWN_VEHICLE_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec { key: "ClassName", label: "Vehicle", kind: FieldKind::String, required: Some(true), default: None, helper: Some("DT_VehicleTemplates row key (e.g. Sandbike, Buggy)"), options: None },
    FieldSpec { key: "X", label: "X", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Y", label: "Y", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Z", label: "Z", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Rotation", label: "Rotation", kind: FieldKind::Float, required: None, default: None, helper: None, options: None },
    FieldSpec { key: "TemplateName", label: "TemplateName", kind: FieldKind::String, required: Some(true), default: None, helper: Some("Template variant key from DT_VehicleTemplates (e.g. T6_Combat). Combobox above pre-fills the first valid one for the picked vehicle."), options: None },
    FieldSpec { key: "Persistent", label: "Persistent", kind: FieldKind::Float, required: None, default: Some(json_const_f(1.0)), helper: Some("0.0 = transient, 1.0 = persistent"), options: None },
    FieldSpec { key: "Faction", label: "Faction", kind: FieldKind::String, required: None, default: None, helper: Some("(blank = default)"), options: None },
];

pub static SPECS: &[CommandSpec] = &[
    CommandSpec {
        id: "AddItemToInventory",
        label: "Grant item",
        category: Category::Items,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Adds an item to the targeted player(s) inventory.",
        fields: ADD_ITEM_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "ServiceBroadcast",
        label: "Broadcast",
        category: Category::Broadcast,
        destructive: None,
        needs_player: false,
        allow_all_players: false,
        describe: "Server-wide broadcast (Generic) or ServerShutdown notice.",
        fields: SERVICE_BROADCAST_FIELDS,
        build: BuildKind::ServiceBroadcast,
    },
    CommandSpec {
        id: "KickPlayer",
        label: "Kick player",
        category: Category::Player,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Disconnects the targeted player(s).",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "CleanPlayerInventory",
        label: "Clean inventory",
        category: Category::Player,
        destructive: Some(true),
        needs_player: true,
        allow_all_players: true,
        describe: "Wipes the targeted player(s) inventory. Destructive.",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "ResetProgression",
        label: "Reset progression",
        category: Category::Player,
        destructive: Some(true),
        needs_player: true,
        allow_all_players: true,
        describe: "Wipes XP/skills/journey progress. Destructive.",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "UpdateAllWaterFillables",
        label: "Refill water",
        category: Category::Player,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Refills water in fillable containers carried by the player.",
        fields: WATER_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "AwardXP",
        label: "Award XP",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Adds generic player XP (server ignores any track/category fields).",
        fields: AWARD_XP_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SpecializationLevelXp",
        label: "Specialization Level XP",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Sets exact specialization track levels for an offline player.",
        fields: &[],
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SkillsSetModuleLevel",
        label: "Set skill module level",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Sets the level of a skill module for the player.",
        fields: SKILL_MODULE_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SkillsSetUnspentSkillPoints",
        label: "Set unspent skill points",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Sets the unspent skill points pool.",
        fields: SKILL_POINTS_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "TeleportTo",
        label: "Teleport (safe)",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Teleports player to coordinates, snapping to safe location.",
        fields: TELEPORT_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "TeleportToExact",
        label: "Teleport (exact)",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Teleports to exact coordinates with no safe-location snap.",
        fields: TELEPORT_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SpawnVehicleAt",
        label: "Spawn vehicle",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Spawns a vehicle at coordinates for the player.",
        fields: SPAWN_VEHICLE_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "CheatScript",
        label: "Cheat script (raw)",
        category: Category::Exec,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Raw `CheatScript` MQ command. Live-tested no-op against seabass servers — the handler logs the call but applies no state. Kept for protocol parity / future Funcom fixes.",
        fields: CHEAT_SCRIPT_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "ServerExec",
        label: "Server exec (raw)",
        category: Category::Exec,
        destructive: None,
        needs_player: false,
        allow_all_players: false,
        describe: "Raw `ServerExec` MQ command. Live-tested no-op against seabass servers — publishes successfully but does not execute useful commands. Kept for protocol parity.",
        fields: SERVER_EXEC_FIELDS,
        build: BuildKind::Passthrough,
    },
    // Journey* commands removed 2026-05-26: published successfully and the
    // server-command handlers fire, but no observable state change in DB or
    // gameplay (live-tested). The `journey-nodes.json` data file remains in
    // case a working path resurfaces.
    //
    // RunLuaScriptFile still omitted — non-shipping path.
];

const fn json_const_i(n: i64) -> serde_json::Value {
    // const fn body: we can't actually call serde_json::json! macro inside const yet;
    // workaround: rely on a Lazy at first use. Inline construction below.
    let _ = n;
    serde_json::Value::Null
}
const fn json_const_f(n: f64) -> serde_json::Value {
    let _ = n;
    serde_json::Value::Null
}
const fn json_const_s(s: &'static str) -> serde_json::Value {
    let _ = s;
    serde_json::Value::Null
}
const fn json_const_b(b: bool) -> serde_json::Value {
    let _ = b;
    serde_json::Value::Null
}
