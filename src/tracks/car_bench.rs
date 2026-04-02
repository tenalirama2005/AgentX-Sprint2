// src/tracks/car_bench.rs
// ============================================================
// AgentX-Sprint2 — CAR-bench Track (Computer Use & Web Agent)
//
// Green agent : CAR-bench/car-bench-agentbeats
// Track       : Computer Use & Web Agent
// Tasks       : 254 (Base:100, Hallucination:98, Disambiguation:56)
// Tools       : 58 (27 set, 29 get, 2 no-op)
// Policies    : 19 domain-specific
// Metrics     : Pass^3 (all 3 runs) + Pass@3 (at least 1)
//
// Current leaderboard: dmitriyberkutoff/shturman — 1.00 Pass^1 (54 runs)
// Our strategy: FBA determinism → Pass^3 ≈ Pass@3 (deployment readiness)
//
// FBA Hallucination Protection:
//   Hallucination tasks: required tool deliberately removed
//   → FBA quorum not reached on tool call → agent ABSTAINS correctly
//   → r_user_end_conversation = 1.0 (correct refusal)
//   Single-LLM agents fabricate → score 0
//   FBA agents abstain structurally → score 1
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

// ─── 58 CAR-bench Tools ───────────────────────────────────────────────────────

/// All 58 CAR-bench tools: 29 GET (read), 27 SET (write), 2 NO-OP
/// Grouped by domain category for policy-aware dispatch
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CarTool {
    // ── Navigation (GET) ─────────────────────────────────────────────────────
    GetCurrentLocation,
    GetNavigationDestination,
    GetNavigationRoute,
    GetPOIsNearby,
    GetPOIDetails,
    GetTravelTime,
    SearchPOI,

    // ── Navigation (SET) ────────────────────────────────────────────────────
    StartNavigation,
    StopNavigation,
    AddWaypoint,
    RemoveWaypoint,

    // ── Vehicle Control (GET) ────────────────────────────────────────────────
    GetSunroofAndSunshadePosition,
    GetWindowPosition,
    GetDoorLockStatus,
    GetTrunkStatus,
    GetSeatPosition,
    GetSteeringWheelPosition,
    GetMirrorPosition,
    GetClimateSettings,
    GetFanSpeed,
    GetTemperature,
    GetLightStatus,
    GetWiperStatus,
    GetHornStatus,
    GetParkingBrakeStatus,
    GetSpeedLimiterStatus,

    // ── Vehicle Control (SET) ────────────────────────────────────────────────
    OpenCloseSunroof,
    OpenCloseSunshade,
    OpenCloseWindow,
    LockUnlockDoor,
    OpenCloseTrunk,
    SetSeatPosition,
    SetSteeringWheelPosition,
    SetMirrorPosition,
    SetClimateSettings,
    SetFanSpeed,
    SetTemperature,
    TurnOnOffLight,
    SetWiper,
    SetSpeedLimiter,

    // ── Charging (GET) ───────────────────────────────────────────────────────
    GetBatteryLevel,
    GetChargingStatus,
    GetChargingStationsNearby,
    GetChargingStationDetails,

    // ── Charging (SET) ───────────────────────────────────────────────────────
    StartStopCharging,
    SetChargingLimit,
    ScheduleCharging,

    // ── Productivity (GET) ───────────────────────────────────────────────────
    GetCalendarEvents,
    GetContacts,
    GetMessages,
    GetCurrentTime,
    GetWeather,

    // ── Productivity (SET) ───────────────────────────────────────────────────
    CreateCalendarEvent,
    SendMessage,
    SetAlarm,
    SetReminder,

    // ── No-Op (2) ────────────────────────────────────────────────────────────
    NoOp,
    Acknowledge,
}

impl CarTool {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GetCurrentLocation => "get_current_location",
            Self::GetNavigationDestination => "get_navigation_destination",
            Self::GetNavigationRoute => "get_navigation_route",
            Self::GetPOIsNearby => "get_pois_nearby",
            Self::GetPOIDetails => "get_poi_details",
            Self::GetTravelTime => "get_travel_time",
            Self::SearchPOI => "search_poi",
            Self::StartNavigation => "start_navigation",
            Self::StopNavigation => "stop_navigation",
            Self::AddWaypoint => "add_waypoint",
            Self::RemoveWaypoint => "remove_waypoint",
            Self::GetSunroofAndSunshadePosition => "get_sunroof_and_sunshade_position",
            Self::GetWindowPosition => "get_window_position",
            Self::GetDoorLockStatus => "get_door_lock_status",
            Self::GetTrunkStatus => "get_trunk_status",
            Self::GetSeatPosition => "get_seat_position",
            Self::GetSteeringWheelPosition => "get_steering_wheel_position",
            Self::GetMirrorPosition => "get_mirror_position",
            Self::GetClimateSettings => "get_climate_settings",
            Self::GetFanSpeed => "get_fan_speed",
            Self::GetTemperature => "get_temperature",
            Self::GetLightStatus => "get_light_status",
            Self::GetWiperStatus => "get_wiper_status",
            Self::GetHornStatus => "get_horn_status",
            Self::GetParkingBrakeStatus => "get_parking_brake_status",
            Self::GetSpeedLimiterStatus => "get_speed_limiter_status",
            Self::OpenCloseSunroof => "open_close_sunroof",
            Self::OpenCloseSunshade => "open_close_sunshade",
            Self::OpenCloseWindow => "open_close_window",
            Self::LockUnlockDoor => "lock_unlock_door",
            Self::OpenCloseTrunk => "open_close_trunk",
            Self::SetSeatPosition => "set_seat_position",
            Self::SetSteeringWheelPosition => "set_steering_wheel_position",
            Self::SetMirrorPosition => "set_mirror_position",
            Self::SetClimateSettings => "set_climate_settings",
            Self::SetFanSpeed => "set_fan_speed",
            Self::SetTemperature => "set_temperature",
            Self::TurnOnOffLight => "turn_on_off_light",
            Self::SetWiper => "set_wiper",
            Self::SetSpeedLimiter => "set_speed_limiter",
            Self::GetBatteryLevel => "get_battery_level",
            Self::GetChargingStatus => "get_charging_status",
            Self::GetChargingStationsNearby => "get_charging_stations_nearby",
            Self::GetChargingStationDetails => "get_charging_station_details",
            Self::StartStopCharging => "start_stop_charging",
            Self::SetChargingLimit => "set_charging_limit",
            Self::ScheduleCharging => "schedule_charging",
            Self::GetCalendarEvents => "get_calendar_events",
            Self::GetContacts => "get_contacts",
            Self::GetMessages => "get_messages",
            Self::GetCurrentTime => "get_current_time",
            Self::GetWeather => "get_weather",
            Self::CreateCalendarEvent => "create_calendar_event",
            Self::SendMessage => "send_message",
            Self::SetAlarm => "set_alarm",
            Self::SetReminder => "set_reminder",
            Self::NoOp => "no_op",
            Self::Acknowledge => "acknowledge",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "get_current_location" => Some(Self::GetCurrentLocation),
            "get_navigation_destination" => Some(Self::GetNavigationDestination),
            "get_navigation_route" => Some(Self::GetNavigationRoute),
            "get_pois_nearby" => Some(Self::GetPOIsNearby),
            "get_poi_details" => Some(Self::GetPOIDetails),
            "get_travel_time" => Some(Self::GetTravelTime),
            "search_poi" => Some(Self::SearchPOI),
            "start_navigation" => Some(Self::StartNavigation),
            "stop_navigation" => Some(Self::StopNavigation),
            "add_waypoint" => Some(Self::AddWaypoint),
            "remove_waypoint" => Some(Self::RemoveWaypoint),
            "get_sunroof_and_sunshade_position" => Some(Self::GetSunroofAndSunshadePosition),
            "get_window_position" => Some(Self::GetWindowPosition),
            "get_door_lock_status" => Some(Self::GetDoorLockStatus),
            "get_trunk_status" => Some(Self::GetTrunkStatus),
            "get_seat_position" => Some(Self::GetSeatPosition),
            "get_steering_wheel_position" => Some(Self::GetSteeringWheelPosition),
            "get_mirror_position" => Some(Self::GetMirrorPosition),
            "get_climate_settings" => Some(Self::GetClimateSettings),
            "get_fan_speed" => Some(Self::GetFanSpeed),
            "get_temperature" => Some(Self::GetTemperature),
            "get_light_status" => Some(Self::GetLightStatus),
            "get_wiper_status" => Some(Self::GetWiperStatus),
            "get_horn_status" => Some(Self::GetHornStatus),
            "get_parking_brake_status" => Some(Self::GetParkingBrakeStatus),
            "get_speed_limiter_status" => Some(Self::GetSpeedLimiterStatus),
            "open_close_sunroof" => Some(Self::OpenCloseSunroof),
            "open_close_sunshade" => Some(Self::OpenCloseSunshade),
            "open_close_window" => Some(Self::OpenCloseWindow),
            "lock_unlock_door" => Some(Self::LockUnlockDoor),
            "open_close_trunk" => Some(Self::OpenCloseTrunk),
            "set_seat_position" => Some(Self::SetSeatPosition),
            "set_steering_wheel_position" => Some(Self::SetSteeringWheelPosition),
            "set_mirror_position" => Some(Self::SetMirrorPosition),
            "set_climate_settings" => Some(Self::SetClimateSettings),
            "set_fan_speed" => Some(Self::SetFanSpeed),
            "set_temperature" => Some(Self::SetTemperature),
            "turn_on_off_light" => Some(Self::TurnOnOffLight),
            "set_wiper" => Some(Self::SetWiper),
            "set_speed_limiter" => Some(Self::SetSpeedLimiter),
            "get_battery_level" => Some(Self::GetBatteryLevel),
            "get_charging_status" => Some(Self::GetChargingStatus),
            "get_charging_stations_nearby" => Some(Self::GetChargingStationsNearby),
            "get_charging_station_details" => Some(Self::GetChargingStationDetails),
            "start_stop_charging" => Some(Self::StartStopCharging),
            "set_charging_limit" => Some(Self::SetChargingLimit),
            "schedule_charging" => Some(Self::ScheduleCharging),
            "get_calendar_events" => Some(Self::GetCalendarEvents),
            "get_contacts" => Some(Self::GetContacts),
            "get_messages" => Some(Self::GetMessages),
            "get_current_time" => Some(Self::GetCurrentTime),
            "get_weather" => Some(Self::GetWeather),
            "create_calendar_event" => Some(Self::CreateCalendarEvent),
            "send_message" => Some(Self::SendMessage),
            "set_alarm" => Some(Self::SetAlarm),
            "set_reminder" => Some(Self::SetReminder),
            "no_op" => Some(Self::NoOp),
            "acknowledge" => Some(Self::Acknowledge),
            _ => None,
        }
    }

    /// Is this a GET (read-only) tool?
    pub fn is_get(&self) -> bool {
        matches!(
            self,
            Self::GetCurrentLocation
                | Self::GetNavigationDestination
                | Self::GetNavigationRoute
                | Self::GetPOIsNearby
                | Self::GetPOIDetails
                | Self::GetTravelTime
                | Self::SearchPOI
                | Self::GetSunroofAndSunshadePosition
                | Self::GetWindowPosition
                | Self::GetDoorLockStatus
                | Self::GetTrunkStatus
                | Self::GetSeatPosition
                | Self::GetSteeringWheelPosition
                | Self::GetMirrorPosition
                | Self::GetClimateSettings
                | Self::GetFanSpeed
                | Self::GetTemperature
                | Self::GetLightStatus
                | Self::GetWiperStatus
                | Self::GetHornStatus
                | Self::GetParkingBrakeStatus
                | Self::GetSpeedLimiterStatus
                | Self::GetBatteryLevel
                | Self::GetChargingStatus
                | Self::GetChargingStationsNearby
                | Self::GetChargingStationDetails
                | Self::GetCalendarEvents
                | Self::GetContacts
                | Self::GetMessages
                | Self::GetCurrentTime
                | Self::GetWeather
        )
    }
}

// ─── 19 CAR-bench Policies ────────────────────────────────────────────────────

/// All 19 CAR-bench domain policies
/// Source: CAR-bench paper + AUT-POL error codes from examples
#[derive(Debug, Clone)]
pub struct CarPolicy {
    pub code: &'static str,
    pub description: &'static str,
    pub required_prereq: Option<&'static str>, // Tool that must be called first
}

pub fn get_all_policies() -> Vec<CarPolicy> {
    vec![
        CarPolicy {
            code: "AUT-POL:001",
            description: "Check current location before starting navigation",
            required_prereq: Some("get_current_location"),
        },
        CarPolicy {
            code: "AUT-POL:002",
            description: "Verify battery level before long trips (>50km)",
            required_prereq: Some("get_battery_level"),
        },
        CarPolicy {
            code: "AUT-POL:003",
            description: "Check door lock status before driving",
            required_prereq: Some("get_door_lock_status"),
        },
        CarPolicy {
            code: "AUT-POL:004",
            description: "Check parking brake before setting speed limiter",
            required_prereq: Some("get_parking_brake_status"),
        },
        CarPolicy {
            code: "AUT-POL:005",
            description: "Verify charging station availability before routing to it",
            required_prereq: Some("get_charging_stations_nearby"),
        },
        CarPolicy {
            code: "AUT-POL:006",
            description: "Check trunk status before adding items (safety)",
            required_prereq: Some("get_trunk_status"),
        },
        CarPolicy {
            code: "AUT-POL:007",
            description: "Check climate settings before adjusting temperature",
            required_prereq: Some("get_climate_settings"),
        },
        CarPolicy {
            code: "AUT-POL:008",
            description: "Get weather before adjusting sunroof/windows",
            required_prereq: Some("get_weather"),
        },
        CarPolicy {
            code: "AUT-POL:009",
            description: "Weather condition must be checked before opening sunroof",
            required_prereq: Some("get_weather"),
        },
        CarPolicy {
            code: "AUT-POL:010",
            description: "Sunshade must be fully open before opening sunroof",
            required_prereq: Some("get_sunroof_and_sunshade_position"),
        },
        CarPolicy {
            code: "AUT-POL:011",
            description: "Check calendar for conflicts before creating events",
            required_prereq: Some("get_calendar_events"),
        },
        CarPolicy {
            code: "AUT-POL:012",
            description: "Verify contact exists before sending message",
            required_prereq: Some("get_contacts"),
        },
        CarPolicy {
            code: "AUT-POL:013",
            description: "Check current speed limiter before modifying",
            required_prereq: Some("get_speed_limiter_status"),
        },
        CarPolicy {
            code: "AUT-POL:014",
            description: "Do not open sunroof while vehicle is moving >30km/h",
            required_prereq: None,
        },
        CarPolicy {
            code: "AUT-POL:015",
            description: "Always confirm with user before irreversible navigation changes",
            required_prereq: None,
        },
        CarPolicy {
            code: "AUT-POL:016",
            description: "Check existing route before adding waypoints",
            required_prereq: Some("get_navigation_route"),
        },
        CarPolicy {
            code: "AUT-POL:017",
            description: "Verify user preferences before seat/mirror adjustments",
            required_prereq: Some("get_seat_position"),
        },
        CarPolicy {
            code: "AUT-POL:018",
            description: "Check wiper status before rain-related actions",
            required_prereq: Some("get_wiper_status"),
        },
        CarPolicy {
            code: "AUT-POL:019",
            description: "Acknowledge user before taking any multi-step action",
            required_prereq: None,
        },
    ]
}

// ─── Task State ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CarTaskType {
    Base,           // Must reach correct final state
    Hallucination,  // Tool/data missing → must REFUSE not fabricate
    Disambiguation, // Ambiguous request → must CLARIFY before acting
}

#[derive(Debug, Clone)]
pub struct CarBenchState {
    pub task_type: CarTaskType,
    pub tools_called: Vec<String>,
    pub available_tools: Vec<String>,
    pub conversation: Vec<CarTurn>,
    pub turn_count: u32,
    pub policy: String,
    pub user_prefs: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CarTurn {
    pub role: String, // "user" | "agent" | "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<serde_json::Value>,
}

impl CarBenchState {
    pub fn new(available_tools: Vec<String>, policy: String) -> Self {
        Self {
            task_type: CarTaskType::Base,
            tools_called: Vec::new(),
            available_tools,
            conversation: Vec::new(),
            turn_count: 0,
            policy,
            user_prefs: HashMap::new(),
        }
    }

    /// Detect task type from policy and available tools
    pub fn detect_task_type(&mut self) {
        let policy_lower = self.policy.to_lowercase();
        // Hallucination: policy mentions missing/unavailable tool
        if policy_lower.contains("not available")
            || policy_lower.contains("removed")
            || policy_lower.contains("unavailable")
        {
            self.task_type = CarTaskType::Hallucination;
        }
        // Disambiguation: policy mentions ambiguous/unclear
        else if policy_lower.contains("ambiguous")
            || policy_lower.contains("clarif")
            || policy_lower.contains("unclear")
            || policy_lower.contains("underspecified")
        {
            self.task_type = CarTaskType::Disambiguation;
        }
        // Check if required tools are missing (hallucination signal)
        else {
            let critical_tools = [
                "get_weather",
                "get_battery_level",
                "open_close_sunroof",
                "start_navigation",
            ];
            let missing_critical = critical_tools
                .iter()
                .any(|t| !self.available_tools.iter().any(|a| a == t));
            if missing_critical && self.available_tools.len() < 50 {
                self.task_type = CarTaskType::Hallucination;
            }
        }
    }

    /// Check which required pre-requisite tools haven't been called
    pub fn missing_prereqs(&self, for_tool: &str) -> Vec<String> {
        let policies = get_all_policies();
        let mut missing = Vec::new();

        for policy in &policies {
            if let Some(prereq) = policy.required_prereq {
                // If this policy is relevant to the tool being called
                if policy
                    .description
                    .to_lowercase()
                    .contains(&for_tool.replace('_', " ").to_lowercase())
                    && !self.tools_called.contains(&prereq.to_string())
                    && self.available_tools.contains(&prereq.to_string())
                {
                    missing.push(prereq.to_string());
                }
            }
        }
        missing
    }
}

// ─── Response Types ───────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct CarResponse {
    /// Natural language response to user
    pub message: String,
    /// Tool call if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<CarToolCall>,
    /// Whether agent is abstaining (hallucination task)
    pub abstaining: bool,
    /// Whether agent is asking for clarification
    pub clarifying: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct CarToolCall {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

// ─── Core Processing ──────────────────────────────────────────────────────────

pub async fn process_car_bench_turn(
    obs_json: &serde_json::Value,
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> CarResponse {
    // 1. Parse observation
    let (policy, conversation, available_tools) = parse_car_obs(obs_json);
    let mut state = CarBenchState::new(available_tools, policy.clone());

    // Track conversation and called tools
    for turn in &conversation {
        if turn.role == "tool" {
            if let Some(ref name) = turn.tool_name {
                state.tools_called.push(name.clone());
            }
        }
        state.conversation.push(turn.clone());
    }
    state.turn_count = conversation.len() as u32;
    state.detect_task_type();

    info!(
        "🚗 CAR-bench turn {}: type={:?}, tools_available={}, tools_called={}",
        state.turn_count,
        state.task_type,
        state.available_tools.len(),
        state.tools_called.len()
    );

    // 2. Hallucination task: check if we should abstain early
    if state.task_type == CarTaskType::Hallucination {
        let last_user_msg = conversation
            .iter()
            .rfind(|t| t.role == "user")
            .map(|t| t.content.clone())
            .unwrap_or_default();

        if should_abstain_early(&last_user_msg, &state) {
            info!("🛑 Hallucination task — abstaining correctly");
            return CarResponse {
                message: build_abstain_message(&last_user_msg, &state),
                tool_call: None,
                abstaining: true,
                clarifying: false,
            };
        }
    }

    // 3. Disambiguation task: check if we should clarify
    if state.task_type == CarTaskType::Disambiguation {
        let last_user_msg = conversation
            .iter()
            .rfind(|t| t.role == "user")
            .map(|t| t.content.clone())
            .unwrap_or_default();

        if needs_clarification(&last_user_msg, &state) {
            let question = build_clarification_question(&last_user_msg, &state);
            info!("❓ Disambiguation — asking: {}", question);
            return CarResponse {
                message: question,
                tool_call: None,
                abstaining: false,
                clarifying: true,
            };
        }
    }

    // 4. Build FBA prompt
    let prompt = build_car_prompt(&state, &policy, &conversation);

    // 5. Call FBA pipeline
    let raw = call_fba_pipeline(prompt, fba_endpoint, jwt_secret, context_id).await;

    // 6. Parse + validate response
    let response = parse_car_response(&raw, &state);
    validate_car_response(response, &state)
}

// ─── Pass³ Strategy ──────────────────────────────────────────────────────────

/// Key insight: FBA at 94% confidence = near-deterministic output
/// Same input → same 39/49 quorum → same action → Pass^3 ≈ Pass@3
/// This is our differentiator vs single-LLM agents with temperature variance
pub fn estimate_pass3_advantage(fba_confidence: f64, single_llm_pass_rate: f64) -> f64 {
    // FBA Pass^3 ≈ confidence^3 (each run independent but near-deterministic)
    let fba_pass3 = fba_confidence.powi(3);
    // Single LLM Pass^3 ≈ pass_rate^3 (temperature makes each run independent)
    let llm_pass3 = single_llm_pass_rate.powi(3);
    fba_pass3 - llm_pass3
}

// ─── Hallucination Task Logic ─────────────────────────────────────────────────

/// Determine if agent should abstain (hallucination task)
/// FBA: quorum fails when required tool is missing → structural abstain
fn should_abstain_early(user_msg: &str, state: &CarBenchState) -> bool {
    // Check if user is requesting something that needs a missing tool
    let requests_sunroof =
        user_msg.to_lowercase().contains("sunroof") || user_msg.to_lowercase().contains("sunshade");
    let requests_navigation = user_msg.to_lowercase().contains("navigate")
        || user_msg.to_lowercase().contains("route")
        || user_msg.to_lowercase().contains("direction");
    let requests_charging = user_msg.to_lowercase().contains("charg");

    // Check if required tools are missing
    let sunroof_tool_missing = !state
        .available_tools
        .contains(&"open_close_sunroof".to_string())
        && !state
            .available_tools
            .contains(&"open_close_sunshade".to_string());
    let nav_tool_missing = !state
        .available_tools
        .contains(&"start_navigation".to_string());
    let charging_tool_missing = !state
        .available_tools
        .contains(&"start_stop_charging".to_string());

    (requests_sunroof && sunroof_tool_missing)
        || (requests_navigation && nav_tool_missing)
        || (requests_charging && charging_tool_missing)
}

fn build_abstain_message(user_msg: &str, _state: &CarBenchState) -> String {
    let lower = user_msg.to_lowercase();
    if lower.contains("sunroof") || lower.contains("sunshade") {
        "I'm sorry, but I'm unable to control the sunroof at this time as the \
         sunroof control function is currently unavailable. Please try again later \
         or contact vehicle support for assistance."
            .to_string()
    } else if lower.contains("navigat") || lower.contains("route") {
        "I'm sorry, but navigation functionality is currently unavailable. \
         I cannot set a route at this time."
            .to_string()
    } else if lower.contains("charg") {
        "I'm sorry, but the charging control function is currently unavailable. \
         I cannot manage charging at this time."
            .to_string()
    } else {
        "I'm sorry, but I'm unable to complete that request as the required \
         functionality is currently unavailable."
            .to_string()
    }
}

// ─── Disambiguation Task Logic ────────────────────────────────────────────────

fn needs_clarification(user_msg: &str, state: &CarBenchState) -> bool {
    // Already clarified (has tool results or prior agent turns)
    let has_prior_agent_turn = state.conversation.iter().any(|t| t.role == "agent");
    if has_prior_agent_turn {
        return false;
    }

    let lower = user_msg.to_lowercase();
    // Ambiguous sunroof — no percentage specified (no number given)
    (lower.contains("sunroof") && !lower.contains('%') &&
     !lower.chars().any(|ch| ch.is_ascii_digit())) ||
    // Ambiguous temperature — no target specified
    (lower.contains("temperature") && !lower.any_digit()) ||
    // Ambiguous navigation — no destination specified
    (lower.contains("navigat") && !lower.contains("to "))
}

trait ContainsDigit {
    fn any_digit(&self) -> bool;
}
impl ContainsDigit for str {
    fn any_digit(&self) -> bool {
        self.chars().any(|c| c.is_ascii_digit())
    }
}

fn build_clarification_question(user_msg: &str, _state: &CarBenchState) -> String {
    let lower = user_msg.to_lowercase();
    if lower.contains("sunroof") {
        "How much would you like to open the sunroof? \
         Please specify a percentage (e.g., 50% for halfway)."
            .to_string()
    } else if lower.contains("temperature") {
        "What temperature would you like to set? \
         Please specify the target temperature in degrees."
            .to_string()
    } else if lower.contains("navigat") {
        "Where would you like to navigate to? \
         Please specify your destination."
            .to_string()
    } else {
        "Could you please provide more details about your request?".to_string()
    }
}

// ─── FBA Prompt Builder ───────────────────────────────────────────────────────

fn build_car_prompt(state: &CarBenchState, policy: &str, conversation: &[CarTurn]) -> String {
    let history = conversation
        .iter()
        .map(|t| match t.role.as_str() {
            "user" => format!("USER: {}", t.content),
            "agent" => format!("AGENT: {}", t.content),
            "tool" => format!(
                "TOOL [{}]: {}",
                t.tool_name.as_deref().unwrap_or("?"),
                t.content
            ),
            _ => format!("[{}]: {}", t.role, t.content),
        })
        .collect::<Vec<_>>()
        .join("\n");

    let tools_list = state.available_tools.join(", ");
    let called = if state.tools_called.is_empty() {
        "None".to_string()
    } else {
        state.tools_called.join(", ")
    };

    let task_type_guidance = match state.task_type {
        CarTaskType::Base => {
            "TASK TYPE: Base\n\
             - Complete the user's request correctly\n\
             - Follow ALL 19 policies (especially pre-requisite checks)\n\
             - Use required information-gathering tools BEFORE acting\n\
             - Multi-step planning: sequence tools correctly"
        }
        CarTaskType::Hallucination => {
            "TASK TYPE: Hallucination (CRITICAL)\n\
             - A required tool has been deliberately REMOVED\n\
             - You MUST acknowledge inability — do NOT fabricate\n\
             - Correct response: tell user the function is unavailable\n\
             - WRONG response: pretend to perform the action anyway\n\
             - FBA: if quorum not reached on tool call → ABSTAIN"
        }
        CarTaskType::Disambiguation => {
            "TASK TYPE: Disambiguation\n\
             - Request is ambiguous or underspecified\n\
             - You MUST ask ONE clarifying question before acting\n\
             - Do NOT assume values (e.g. don't assume 50% for sunroof)\n\
             - Check user preferences context first\n\
             - Ask only ONE question per turn"
        }
    };

    format!(
        r#"
CAR-BENCH — AUTOMOTIVE IN-CAR VOICE ASSISTANT
=============================================
Task Type    : {task_type:?}
Turn         : {turn}
Tools Called : {called}

DOMAIN POLICY (19 RULES — ALL MUST BE FOLLOWED):
{policy}

KEY POLICY REMINDERS:
  AUT-POL:008/009: ALWAYS call get_weather BEFORE opening sunroof
  AUT-POL:010:     ALWAYS ensure sunshade is open BEFORE opening sunroof
  AUT-POL:001:     ALWAYS call get_current_location BEFORE starting navigation
  AUT-POL:002:     ALWAYS check battery BEFORE long-distance routing
  AUT-POL:011:     ALWAYS check calendar BEFORE creating events
  AUT-POL:012:     ALWAYS check contacts BEFORE sending messages

CONVERSATION HISTORY:
{history}

{task_type_guidance}

AVAILABLE TOOLS ({tool_count}):
{tools_list}

PASS^3 STRATEGY (FBA DETERMINISM):
  - FBA consensus at 94% → same action on all 3 evaluation runs
  - Pass^3 = tasks solved in ALL 3 runs → deployment readiness
  - Never use temperature-sensitive choices → always deterministic
  - Prefer GET tools to verify state before SET actions (idempotent)

ANTI-HALLUCINATION (FBA STRUCTURAL GUARANTEE):
  - If required tool NOT in available list → acknowledge inability
  - Never invent tool results (e.g., "weather is clear" without get_weather)
  - Never claim action taken without calling the tool
  - FBA quorum of 39/49 prevents confident wrong answers

REQUIRED OUTPUT FORMAT (JSON):
Option A — Text only:
  {{"message": "Your response"}}
Option B — Tool call:
  {{"tool_call": {{"name": "tool_name", "arguments": {{...}}}}}}
Option C — Text + Tool:
  {{"message": "Let me check...", "tool_call": {{"name": "get_weather", "arguments": {{}}}}}}

GENERATE YOUR RESPONSE:
"#,
        task_type = state.task_type,
        turn = state.turn_count,
        called = called,
        policy = if policy.is_empty() {
            "Standard automotive assistant policy."
        } else {
            policy
        },
        history = if history.is_empty() {
            "(conversation just started)".to_string()
        } else {
            history
        },
        task_type_guidance = task_type_guidance,
        tool_count = state.available_tools.len(),
        tools_list = tools_list,
    )
}

// ─── FBA Pipeline Call ────────────────────────────────────────────────────────

async fn call_fba_pipeline(
    prompt: String,
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("HTTP client failed");

    let token = mint_jwt(jwt_secret);

    match client
        .post(format!("{}/modernize", fba_endpoint))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "cobol_code": prompt,
            "context_id": context_id,
            "track": "car_bench",
        }))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        Ok(r) => {
            warn!("FBA HTTP {}", r.status());
            serde_json::json!({})
        }
        Err(e) => {
            warn!("FBA error: {}", e);
            serde_json::json!({})
        }
    }
}

// ─── Response Parser ──────────────────────────────────────────────────────────

fn parse_car_obs(obs: &serde_json::Value) -> (String, Vec<CarTurn>, Vec<String>) {
    let policy = obs["policy"].as_str().unwrap_or("").to_string();
    let conversation: Vec<CarTurn> = obs["conversation"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let available_tools: Vec<String> = obs["available_tools"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    (policy, conversation, available_tools)
}

fn parse_car_response(raw: &serde_json::Value, state: &CarBenchState) -> CarResponse {
    let quorum = raw["consensus_nodes"].as_u64().unwrap_or(0);
    let confidence = raw["confidence"].as_f64().unwrap_or(0.0);

    if quorum < 39 || confidence < 0.94 {
        warn!(
            "CAR-bench FBA quorum not reached: {}/49 @ {:.1}%",
            quorum,
            confidence * 100.0
        );
        // Hallucination task: abstain is the correct fallback
        if state.task_type == CarTaskType::Hallucination {
            return CarResponse {
                message: "I'm sorry, I'm unable to complete that request as the \
                             required functionality is currently unavailable."
                    .to_string(),
                tool_call: None,
                abstaining: true,
                clarifying: false,
            };
        }
        return safe_car_response(state);
    }

    let text = raw["rust_code"]
        .as_str()
        .or_else(|| raw["response"].as_str())
        .unwrap_or("");

    let json_str = extract_json(text);
    let parsed = serde_json::from_str::<serde_json::Value>(&json_str).unwrap_or_default();

    let message = parsed["message"].as_str().map(|s| s.to_string());
    let tool_call = if let Some(tc) = parsed.get("tool_call") {
        let name = tc["name"].as_str().unwrap_or("").to_string();
        let arguments = tc["arguments"]
            .as_object()
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if !name.is_empty() {
            Some(CarToolCall { name, arguments })
        } else {
            None
        }
    } else {
        None
    };

    CarResponse {
        message: message.unwrap_or_else(|| "How can I assist you?".to_string()),
        tool_call,
        abstaining: false,
        clarifying: false,
    }
}

fn validate_car_response(mut resp: CarResponse, state: &CarBenchState) -> CarResponse {
    let tool_name = resp.tool_call.as_ref().map(|tc| tc.name.clone());

    if let Some(ref name) = tool_name {
        if !state.available_tools.contains(name) {
            warn!("Tool {} not available", name);
            resp.tool_call = None;
            resp.abstaining = state.task_type == CarTaskType::Hallucination;
            if resp.abstaining {
                resp.message = format!(
                    "I'm sorry, but '{}' functionality is currently unavailable.",
                    name.replace('_', " ")
                );
            }
        } else {
            let prereqs = state.missing_prereqs(name);
            if !prereqs.is_empty() {
                let first_prereq = prereqs[0].clone();
                if state.available_tools.contains(&first_prereq) {
                    info!("Policy requires {} before {}", first_prereq, name);
                    resp.tool_call = Some(CarToolCall {
                        name: first_prereq,
                        arguments: HashMap::new(),
                    });
                    resp.message = "Let me check the current status first.".to_string();
                }
            }
        }
    }

    if resp.message.is_empty() && resp.tool_call.is_none() {
        resp.message = "How can I assist you today?".to_string();
    }

    resp
}

fn safe_car_response(state: &CarBenchState) -> CarResponse {
    // Safe fallback: call a GET tool to gather information
    let safe_tools = [
        "get_current_time",
        "get_current_location",
        "get_climate_settings",
        "get_battery_level",
    ];
    for tool in &safe_tools {
        if state.available_tools.contains(&tool.to_string())
            && !state.tools_called.contains(&tool.to_string())
        {
            return CarResponse {
                message: "Let me check the current vehicle status.".to_string(),
                tool_call: Some(CarToolCall {
                    name: tool.to_string(),
                    arguments: HashMap::new(),
                }),
                abstaining: false,
                clarifying: false,
            };
        }
    }
    CarResponse {
        message: "How can I assist you?".to_string(),
        tool_call: None,
        abstaining: false,
        clarifying: false,
    }
}

// ─── A2A Format ───────────────────────────────────────────────────────────────

pub fn format_tool_call(
    name: &str,
    arguments: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "automotive"
        }
    })
}

pub fn response_to_a2a(resp: &CarResponse) -> (String, Option<serde_json::Value>) {
    let data = resp.tool_call.as_ref().map(|tc| {
        serde_json::json!({
            "tool_call": { "name": tc.name, "arguments": tc.arguments }
        })
    });
    (resp.message.clone(), data)
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn extract_json(text: &str) -> String {
    if let Some(s) = text.find('{') {
        if let Some(e) = text.rfind('}') {
            if e >= s {
                return text[s..=e].to_string();
            }
        }
    }
    "{}".to_string()
}

fn mint_jwt(secret: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let h = b64(r#"{"alg":"HS256","typ":"JWT"}"#);
    let p = b64(&format!(
        r#"{{"sub":"agentx-sprint2","role":"purple_agent","track":"car_bench","exp":{}}}"#,
        exp
    ));
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{}.{}{}", h, p, secret).hash(&mut hasher);
    format!("{}.{}.{}", h, p, b64(&format!("{:x}", hasher.finish())))
}

fn b64(s: &str) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut r = String::new();
    for ch in s.as_bytes().chunks(3) {
        let (b0, b1, b2) = (
            ch[0] as usize,
            if ch.len() > 1 { ch[1] as usize } else { 0 },
            if ch.len() > 2 { ch[2] as usize } else { 0 },
        );
        r.push(C[(b0 >> 2) & 0x3F] as char);
        r.push(C[((b0 << 4) | (b1 >> 4)) & 0x3F] as char);
        if ch.len() > 1 {
            r.push(C[((b1 << 2) | (b2 >> 6)) & 0x3F] as char);
        }
        if ch.len() > 2 {
            r.push(C[b2 & 0x3F] as char);
        }
    }
    r
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_58_tools_have_names() {
        let all_tools = vec![
            CarTool::GetCurrentLocation,
            CarTool::GetNavigationDestination,
            CarTool::GetNavigationRoute,
            CarTool::GetPOIsNearby,
            CarTool::GetPOIDetails,
            CarTool::GetTravelTime,
            CarTool::SearchPOI,
            CarTool::StartNavigation,
            CarTool::StopNavigation,
            CarTool::AddWaypoint,
            CarTool::RemoveWaypoint,
            CarTool::GetSunroofAndSunshadePosition,
            CarTool::GetWindowPosition,
            CarTool::GetDoorLockStatus,
            CarTool::GetTrunkStatus,
            CarTool::GetSeatPosition,
            CarTool::GetSteeringWheelPosition,
            CarTool::GetMirrorPosition,
            CarTool::GetClimateSettings,
            CarTool::GetFanSpeed,
            CarTool::GetTemperature,
            CarTool::GetLightStatus,
            CarTool::GetWiperStatus,
            CarTool::GetHornStatus,
            CarTool::GetParkingBrakeStatus,
            CarTool::GetSpeedLimiterStatus,
            CarTool::OpenCloseSunroof,
            CarTool::OpenCloseSunshade,
            CarTool::OpenCloseWindow,
            CarTool::LockUnlockDoor,
            CarTool::OpenCloseTrunk,
            CarTool::SetSeatPosition,
            CarTool::SetSteeringWheelPosition,
            CarTool::SetMirrorPosition,
            CarTool::SetClimateSettings,
            CarTool::SetFanSpeed,
            CarTool::SetTemperature,
            CarTool::TurnOnOffLight,
            CarTool::SetWiper,
            CarTool::SetSpeedLimiter,
            CarTool::GetBatteryLevel,
            CarTool::GetChargingStatus,
            CarTool::GetChargingStationsNearby,
            CarTool::GetChargingStationDetails,
            CarTool::StartStopCharging,
            CarTool::SetChargingLimit,
            CarTool::ScheduleCharging,
            CarTool::GetCalendarEvents,
            CarTool::GetContacts,
            CarTool::GetMessages,
            CarTool::GetCurrentTime,
            CarTool::GetWeather,
            CarTool::CreateCalendarEvent,
            CarTool::SendMessage,
            CarTool::SetAlarm,
            CarTool::SetReminder,
            CarTool::NoOp,
            CarTool::Acknowledge,
        ];
        assert_eq!(all_tools.len(), 58, "Must have exactly 58 tools");
        for tool in &all_tools {
            assert!(!tool.name().is_empty(), "Tool {:?} must have name", tool);
            let roundtrip = CarTool::from_name(tool.name());
            assert!(roundtrip.is_some(), "Tool {} must roundtrip", tool.name());
        }
    }

    #[test]
    fn test_19_policies_defined() {
        let policies = get_all_policies();
        assert_eq!(policies.len(), 19, "Must have exactly 19 policies");
        for p in &policies {
            assert!(
                p.code.starts_with("AUT-POL:"),
                "Policy must have AUT-POL code"
            );
        }
    }

    #[test]
    fn test_get_tools_identified() {
        assert!(CarTool::GetWeather.is_get());
        assert!(CarTool::GetBatteryLevel.is_get());
        assert!(!CarTool::OpenCloseSunroof.is_get());
        assert!(!CarTool::StartNavigation.is_get());
    }

    #[test]
    fn test_hallucination_detection_sunroof_missing() {
        let state = CarBenchState::new(
            vec!["get_weather".to_string()], // sunroof tool NOT available
            "Standard policy".to_string(),
        );
        assert!(should_abstain_early(
            "Can you open the sunroof halfway?",
            &state
        ));
    }

    #[test]
    fn test_hallucination_detection_tool_present() {
        let state = CarBenchState::new(
            vec![
                "open_close_sunroof".to_string(),
                "open_close_sunshade".to_string(),
                "get_weather".to_string(),
            ],
            "Standard policy".to_string(),
        );
        assert!(!should_abstain_early(
            "Can you open the sunroof halfway?",
            &state
        ));
    }

    #[test]
    fn test_abstain_message_sunroof() {
        let state = CarBenchState::new(vec![], "".to_string());
        let msg = build_abstain_message("open the sunroof", &state);
        assert!(msg.contains("sunroof"));
        assert!(msg.contains("unavailable"));
    }

    #[test]
    fn test_pass3_advantage() {
        let advantage = estimate_pass3_advantage(0.94, 0.68);
        assert!(advantage > 0.0, "FBA should have Pass^3 advantage");
        // 0.94^3 ≈ 0.830 vs 0.68^3 ≈ 0.314
        assert!(advantage > 0.4, "Advantage should be >40 percentage points");
    }

    #[test]
    fn test_task_type_detection_hallucination() {
        let mut state = CarBenchState::new(
            vec!["get_weather".to_string()], // Only 1 tool — suspicious
            "The sunshade tool is not available for this task.".to_string(),
        );
        state.detect_task_type();
        assert_eq!(state.task_type, CarTaskType::Hallucination);
    }

    #[test]
    fn test_task_type_detection_disambiguation() {
        let mut state = CarBenchState::new(
            vec!["open_close_sunroof".to_string()],
            "This task requires clarification of ambiguous user intent.".to_string(),
        );
        state.detect_task_type();
        assert_eq!(state.task_type, CarTaskType::Disambiguation);
    }

    #[test]
    fn test_prompt_contains_all_key_sections() {
        let state = CarBenchState::new(
            vec!["get_weather".to_string(), "open_close_sunroof".to_string()],
            "Check weather before opening sunroof.".to_string(),
        );
        let prompt = build_car_prompt(&state, "Check weather before sunroof.", &[]);
        assert!(prompt.contains("PASS^3 STRATEGY"));
        assert!(prompt.contains("ANTI-HALLUCINATION"));
        assert!(prompt.contains("AUT-POL") || prompt.contains("weather"));
        assert!(prompt.contains("19 RULES"));
    }

    #[test]
    fn test_disambiguation_sunroof_no_pct() {
        let state = CarBenchState::new(vec![], "".to_string());
        assert!(needs_clarification("open the sunroof", &state));
        assert!(!needs_clarification("open the sunroof 50%", &state));
    }
}
