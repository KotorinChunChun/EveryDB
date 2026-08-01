//! Windows の本人確認（Windows Hello: 顔 / 指紋 / PIN、未対応時はフォールバック）。
//!
//! 以前はアプリ独自の「マスターパスワード」（argon2 ハッシュを app_meta に保存）で
//! 秘密情報の表示をゲートしていたが、
//! - トークンの実際の暗号化は Windows DPAPI（現在の Windows ユーザーに紐づく）が行っており、
//!   マスターパスワードは暗号鍵ではなく単なる UI 上の確認でしかない
//! - Windows にログインできている時点で本人確認は済んでおり、その上にアプリ独自のパスワードを
//!   重ねて DB に持つ必然性が無い
//! という理由で廃止し、OS 標準の本人確認（Windows Hello）に置き換えた。
//! 本ツールの DB はパスワード（およびそのハッシュ）を一切保持しない。

use anyhow::Result;

/// Windows Hello が利用可能か（PIN/生体認証が設定済みか）。
/// 利用できない環境（PIN 未設定・非対応ハード・非 Windows）では確認をスキップする方針のため、
/// 呼び出し側はこれが false のとき「確認不要（許可）」として扱う。
#[cfg(windows)]
pub fn is_available() -> bool {
    use windows::Security::Credentials::UI::{
        UserConsentVerifier, UserConsentVerifierAvailability,
    };
    (|| -> Result<bool> {
        let availability = UserConsentVerifier::CheckAvailabilityAsync()?.get()?;
        Ok(availability == UserConsentVerifierAvailability::Available)
    })()
    .unwrap_or(false)
}

/// Windows Hello の確認ダイアログを出す。`hwnd` は親にするウィンドウハンドル。
/// 戻り値 true = 本人確認できた。
#[cfg(windows)]
pub fn verify(hwnd: windows::Win32::Foundation::HWND, message: &str) -> Result<bool> {
    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows_future::IAsyncOperation;

    // デスクトップ（Win32）アプリでは UserConsentVerifier::RequestVerificationAsync は使えず、
    // 親ウィンドウを渡すインターop 経由で呼ぶ必要がある。
    let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?;
    let msg = HSTRING::from(message);
    let op: IAsyncOperation<UserConsentVerificationResult> =
        unsafe { interop.RequestVerificationForWindowAsync(hwnd, &msg)? };
    let result = op.get()?;
    Ok(result == UserConsentVerificationResult::Verified)
}

// 非 Windows（CI 等）では常に「利用不可 → 確認不要」。実運用は Windows 前提。
#[cfg(not(windows))]
pub fn is_available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn verify(_hwnd: (), _message: &str) -> Result<bool> {
    Ok(true)
}
