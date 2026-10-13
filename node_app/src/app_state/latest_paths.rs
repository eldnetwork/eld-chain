//! Fixed `/@eld/.../LATEST` CADO paths, built once.

use eld_common::cado::{CadoPath, CadoPathKey, CadoType};
use eld_common::constants::cado::LATEST;
use std::sync::OnceLock;

fn static_latest_path(ty: CadoType) -> &'static CadoPath {
    static TIP: OnceLock<CadoPath> = OnceLock::new();
    static SNAPSHOT: OnceLock<CadoPath> = OnceLock::new();
    static EPOCH: OnceLock<CadoPath> = OnceLock::new();

    match ty {
        CadoType::AppStateTip => TIP.get_or_init(|| {
            CadoPath::new(CadoType::AppStateTip, CadoPathKey::Name(LATEST))
                .expect("LATEST is a valid AppStateTip name")
        }),
        CadoType::AppStateSnapshot => SNAPSHOT.get_or_init(|| {
            CadoPath::new(CadoType::AppStateSnapshot, CadoPathKey::Name(LATEST))
                .expect("LATEST is a valid AppStateSnapshot name")
        }),
        CadoType::EpochRecord => EPOCH.get_or_init(|| {
            CadoPath::new(CadoType::EpochRecord, CadoPathKey::Name(LATEST))
                .expect("LATEST is a valid EpochRecord name")
        }),
        _ => panic!("not a latest path type"),
    }
}

/// Fixed path for the latest AppStateTip.
pub fn latest_app_state_tip_path() -> &'static CadoPath {
    static_latest_path(CadoType::AppStateTip)
}

/// Fixed path for the latest AppStateSnapshot.
pub fn latest_app_state_snapshot_path() -> &'static CadoPath {
    static_latest_path(CadoType::AppStateSnapshot)
}

/// Fixed path for the latest EpochRecord.
pub fn latest_epoch_record_path() -> &'static CadoPath {
    static_latest_path(CadoType::EpochRecord)
}
