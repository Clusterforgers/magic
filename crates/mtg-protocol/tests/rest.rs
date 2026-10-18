//! Goal: the account bodies keep their documented JSON shape and refuse anything else, a password never reaches `Debug` output, every error code has one spelling shared by serde and `as_str`, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_protocol::bindings::export_to;
use mtg_protocol::rest::{
    AuthErrorCode, Credentials, PASSWORD_MAX_CHARS, PASSWORD_MIN_CHARS, USERNAME_MAX_CHARS,
    USERNAME_MIN_CHARS, UserView,
};
use rstest::rstest;
use serde_json::{Value, json};

/// Index of each code; adding a variant breaks this match until `ALL` and the tables grow.
fn index(code: AuthErrorCode) -> usize {
    match code {
        AuthErrorCode::InvalidRequest => 0,
        AuthErrorCode::InvalidUsername => 1,
        AuthErrorCode::InvalidPassword => 2,
        AuthErrorCode::UsernameTaken => 3,
        AuthErrorCode::RegistrationClosed => 4,
        AuthErrorCode::InvalidCredentials => 5,
        AuthErrorCode::TooManyAttempts => 6,
        AuthErrorCode::Unauthenticated => 7,
        AuthErrorCode::ForbiddenOrigin => 8,
        AuthErrorCode::PayloadTooLarge => 9,
        AuthErrorCode::UnsupportedMediaType => 10,
        AuthErrorCode::AccountsUnavailable => 11,
        AuthErrorCode::Unavailable => 12,
        AuthErrorCode::Internal => 13,
    }
}

#[test]
fn credentials_have_the_documented_wire_shape() {
    let credentials = Credentials {
        username: "Ann_1".into(),
        password: "a long passphrase".into(),
    };
    let value = serde_json::to_value(&credentials).unwrap();
    assert_eq!(
        value,
        json!({"username": "Ann_1", "password": "a long passphrase"})
    );
    assert_eq!(
        serde_json::from_value::<Credentials>(value).unwrap(),
        credentials
    );

    let user = UserView {
        id: "0b3c8a52-3f1e-4a5c-9d0e-6f1a2b3c4d5e".into(),
        username: "Ann_1".into(),
    };
    assert_eq!(
        serde_json::to_value(&user).unwrap(),
        json!({"id": "0b3c8a52-3f1e-4a5c-9d0e-6f1a2b3c4d5e", "username": "Ann_1"})
    );
}

#[rstest]
#[case::missing_password(json!({"username": "ann"}))]
#[case::missing_username(json!({"password": "a long passphrase"}))]
#[case::extra_field(json!({"username": "ann", "password": "a long passphrase", "admin": true}))]
#[case::number_for_password(json!({"username": "ann", "password": 1_234_567_890}))]
#[case::null_username(json!({"username": null, "password": "a long passphrase"}))]
#[case::short_array(json!(["ann"]))]
#[case::long_array(json!(["ann", "a long passphrase", "extra"]))]
#[case::string(json!("ann"))]
#[case::null(json!(null))]
fn credentials_that_are_not_exactly_a_username_and_a_password_are_refused(#[case] body: Value) {
    assert!(serde_json::from_value::<Credentials>(body).is_err());
}

#[rstest]
#[case("hunter2-but-longer")]
#[case("pässwörd with ünicode 日本語")]
#[case("\"quoted\" and \\ escaped")]
fn debug_output_never_shows_the_password(#[case] password: &str) {
    let credentials = Credentials {
        username: "ann".into(),
        password: password.into(),
    };
    for shown in [
        format!("{credentials:?}"),
        format!("{credentials:#?}"),
        format!("{:?}", Some(&credentials)),
        format!("{:?}", vec![credentials.clone()]),
    ] {
        assert!(!shown.contains(password), "{shown}");
        assert!(
            shown.contains("ann") && shown.contains("<redacted>"),
            "{shown}"
        );
    }
}

#[test]
fn error_codes_have_one_spelling_for_serde_and_as_str() {
    let mut seen = BTreeSet::new();
    for (position, code) in AuthErrorCode::ALL.into_iter().enumerate() {
        assert_eq!(index(code), position, "ALL is in declaration order");
        let text = code.as_str();
        assert_eq!(serde_json::to_value(code).unwrap(), json!(text));
        assert_eq!(
            serde_json::from_value::<AuthErrorCode>(json!(text)).unwrap(),
            code
        );
        assert!(
            text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{text} is snake_case"
        );
        assert!(seen.insert(text), "{text} appears twice");
    }
    assert_eq!(seen.len(), AuthErrorCode::ALL.len());
}

#[rstest]
#[case("InvalidCredentials")]
#[case("invalid-credentials")]
#[case("INVALID_CREDENTIALS")]
#[case("")]
#[case("nope")]
fn other_spellings_of_a_code_are_refused(#[case] text: &str) {
    assert!(serde_json::from_value::<AuthErrorCode>(json!(text)).is_err());
}

#[test]
fn the_limits_are_ordered_and_match_the_documented_numbers() {
    assert_eq!((USERNAME_MIN_CHARS, USERNAME_MAX_CHARS), (3, 32));
    assert_eq!((PASSWORD_MIN_CHARS, PASSWORD_MAX_CHARS), (10, 128));
    const { assert!(USERNAME_MIN_CHARS <= USERNAME_MAX_CHARS) };
    const { assert!(PASSWORD_MIN_CHARS <= PASSWORD_MAX_CHARS) };
}

fn generated() -> BTreeMap<String, String> {
    let scratch = tempfile::tempdir().unwrap();
    export_to(scratch.path()).unwrap();
    std::fs::read_dir(scratch.path())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_bindings_carry_the_account_types() {
    let files = generated();
    for needed in ["Credentials.ts", "UserView.ts", "AuthErrorCode.ts"] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["Credentials.ts"].contains("username: string"));
    assert!(files["Credentials.ts"].contains("password: string"));
    assert!(files["UserView.ts"].contains("id: string"));
    // The union lists exactly the codes the server can send.
    for code in AuthErrorCode::ALL {
        assert!(
            files["AuthErrorCode.ts"].contains(&format!("\"{}\"", code.as_str())),
            "{} missing from AuthErrorCode.ts",
            code.as_str()
        );
    }
}
