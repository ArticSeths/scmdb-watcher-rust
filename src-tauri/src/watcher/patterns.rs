use once_cell::sync::Lazy;
use regex::Regex;

pub static RE_MARKER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"CreateMarker.*missionId \[([^\]]+)\].*generator name \[([^\]]+)\].*contract \[([^\]]+)\]",
    )
    .unwrap()
});

pub static RE_ACCEPTED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Added notification "Contract Accepted:.*?MissionId: \[([^\]]+)\]"#).unwrap()
});

pub static RE_END_MISSION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<EndMission>.*MissionId\[([^\]]+)\].*CompletionType\[(\w+)\].*Reason\[([^\]]+)\]")
        .unwrap()
});

pub static RE_BLUEPRINT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Added notification "Received Blueprint: ([^:]+):"#).unwrap());
