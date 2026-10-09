//! What a store leaves out of the Manifest unless asked to keep it
//! (Q15): the encrypted credential strings and the plain-text
//! password an Oracle `exp` command line carries. Every replaced
//! value leaves its SHA-256 behind in `store_redaction`, so the
//! original can still be recognised, never recovered.

use super::input::sha256_hex;

/// How one Manifest field was changed, as `store_redaction.rule` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionRule {
    /// The whole value replaced by its lowercase hex SHA-256.
    Sha256,
    /// The password of a `user/password@service` credential replaced
    /// by `***`; the rest of the value as written.
    PasswordMasked,
    /// The whole value replaced by `***`: a command line whose
    /// credential shape was not recognised.
    ValueMasked,
}

impl RedactionRule {
    /// The label `store_redaction.rule` holds.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::PasswordMasked => "password-masked",
            Self::ValueMasked => "value-masked",
        }
    }
}

/// What replaces a masked password or an unrecognised command.
pub const MASK: &str = "***";

/// The replacement for field `position` (1-based, after the key) of a
/// line keyed `key`, or `None` when the field stays as written.
///
/// * `DBUids`, `DBPwds`: every field becomes its SHA-256.
/// * `SiteConnInfo`, `PlantConnInfo`: fields 1 and 5 (the 64-character
///   encrypted strings; on `PlantConnInfo` field 1 is shorter but
///   treated alike) become their SHA-256.
/// * `BackupCommand`: see [`redact_backup_command`].
pub fn redact_field(key: &str, position: usize, value: &str) -> Option<(String, RedactionRule)> {
    match key {
        "DBUids" | "DBPwds" => Some((sha256_hex(value.as_bytes()), RedactionRule::Sha256)),
        "SiteConnInfo" | "PlantConnInfo" if position == 1 || position == 5 => {
            Some((sha256_hex(value.as_bytes()), RedactionRule::Sha256))
        }
        "BackupCommand" => redact_backup_command(value),
        _ => None,
    }
}

/// The `BackupCommand` value with its password taken out.
///
/// An Oracle `exp` command line carries `user/password@service` as one
/// whitespace-delimited token; the password -- between the token's
/// first `/` and its last `@` -- becomes [`MASK`] and the rest stays.
/// SQL Server's `BACKUP DATABASE <db> TO <device>` carries no
/// credential and stays as written. Any other shape is masked whole,
/// since a password it might carry cannot be told apart from the
/// rest.
pub fn redact_backup_command(value: &str) -> Option<(String, RedactionRule)> {
    for token in value.split_whitespace() {
        let Some(slash) = token.find('/') else {
            continue;
        };
        let Some(at) = token.rfind('@') else {
            continue;
        };
        if at <= slash {
            continue;
        }
        let masked = format!("{}{}{}", &token[..=slash], MASK, &token[at..]);
        return Some((
            value.replacen(token, &masked, 1),
            RedactionRule::PasswordMasked,
        ));
    }
    let upper = value.trim_start().to_ascii_uppercase();
    if upper.starts_with("BACKUP DATABASE ") && !value.contains('/') {
        return None;
    }
    Some((MASK.to_string(), RedactionRule::ValueMasked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_strings_become_their_sha256() {
        let (value, rule) = redact_field("DBPwds", 1, "secret,secret2").unwrap();
        assert_eq!(sha256_hex(b"secret,secret2"), value);
        assert_eq!(RedactionRule::Sha256, rule);
        assert!(redact_field("DBUids", 1, "TEST02,TEST02d").is_some());

        assert!(redact_field("SiteConnInfo", 1, "x").is_some());
        assert!(redact_field("PlantConnInfo", 5, "x").is_some());
        assert_eq!(None, redact_field("PlantConnInfo", 4, "TEST02pid"));
        assert_eq!(None, redact_field("Table", 1, "TEST02pid"));
    }

    #[test]
    fn an_oracle_exp_command_keeps_everything_but_the_password() {
        let command = "Exp.exe system/Pa@ss-w0rd@ORCL CONSISTENT=y OWNER=(SQPlantpidd,SQPlant) \
                       FILE='\\\\mm-128\\Backup\\Export.dmp'";
        let (value, rule) = redact_backup_command(command).unwrap();
        assert_eq!(RedactionRule::PasswordMasked, rule);
        assert_eq!(
            "Exp.exe system/***@ORCL CONSISTENT=y OWNER=(SQPlantpidd,SQPlant) \
             FILE='\\\\mm-128\\Backup\\Export.dmp'",
            value
        );
        assert!(!value.contains("Pa@ss-w0rd"));
    }

    #[test]
    fn a_sql_server_backup_command_carries_no_credential_and_stays() {
        assert_eq!(
            None,
            redact_backup_command("BACKUP DATABASE SP3DTrain_RDB_SCHEMA TO TEST02")
        );
    }

    #[test]
    fn an_unrecognised_command_is_masked_whole() {
        assert_eq!(
            Some((MASK.to_string(), RedactionRule::ValueMasked)),
            redact_backup_command("some-tool --password hunter2")
        );
        // A BACKUP DATABASE line with a slash in it is not the shape we know.
        assert_eq!(
            Some((MASK.to_string(), RedactionRule::ValueMasked)),
            redact_backup_command("BACKUP DATABASE x TO DISK='c:/x' WITH PASSWORD='p'")
        );
    }
}
