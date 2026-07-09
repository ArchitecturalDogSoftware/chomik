// Don't automatically allocate a console when run in Windows.
//
// Given that logs can be desirable, `AttachConsole` is used to attach to the parent console when run from one instead.
#![windows_subsystem = "windows"]

mod animations;
mod settings;
mod window;

fn main() {
    #[cfg(target_os = "windows")]
    self::try_attach_console();

    let mut application = bevy::app::App::new();

    crate::settings::init(&mut application);
    crate::window::init(&mut application);
    crate::animations::init(&mut application);

    application.run();
}

/// Attempt to attach to the console of the parent process, so that logs can be viewed. This is used instead of
/// `windows_subsystem = "console"` to avoid a new console window being opened when the application is run from a GUI.
///
/// This has the downside of not causing the calling shell to wait for the application to exit and of not allowing the
/// shell to stop the application with `ctrl` + `c`, but I feel this is a reasonable tradeoff. This _may_ be solved by
/// the new-ish console allocation policy (see [MS Learn] and the [MS Terminal Specifications] for details), but this
/// isn't implemented by Rust or the `windows` crate, so I haven't investigated this.
///
/// [MS Learn]: <https://learn.microsoft.com/en-us/windows/console/console-allocation-policy>
/// [MS Terminal Specifications]: <https://github.com/microsoft/terminal/blob/9c452cd/doc/specs/%237335%20-%20Console%20Allocation%20Policy.md>
#[cfg(target_os = "windows")]
fn try_attach_console() {
    use windows::Win32::Foundation;
    use windows::Win32::System::Console;

    // SAFETY: Microsoft's documentation does not list any safety requirements, only inputs and contexts which could
    // cause an error to be returned. See <https://learn.microsoft.com/en-us/windows/console/attachconsole>.
    unsafe {
        match Console::AttachConsole(Console::ATTACH_PARENT_PROCESS) {
            // Returned if the parent process doesn't have a console, taken to indicate this has been launched from a
            // GUI and shouldn't have a console.
            Err(e) if Foundation::WIN32_ERROR::from_error(&e) == Some(Foundation::ERROR_INVALID_HANDLE) => (),
            Ok(()) => (),
            Err(e) => {
                self::error_popup(
                    format!("Encountered an error while attempting to attach to a console: {e})").as_str(),
                );

                // In the off chance that this has somewhere to go.
                panic!("encountered an error while attempting to attach to a console: {e}");
            }
        }
    }
}

/// Display an error window with the given message for users.
#[cfg(target_os = "windows")]
fn error_popup(message: &str) {
    use windows::Win32::Foundation;
    use windows::Win32::UI::WindowsAndMessaging;

    const MESSAGE_BOX_ERROR: WindowsAndMessaging::MESSAGEBOX_RESULT = WindowsAndMessaging::MESSAGEBOX_RESULT(0);

    // SAFETY: Microsoft's documentation does not list any safety requirements, only the potential that it may return an
    // error. An obvious safety requirement, however, is that the strings are valid, which would be upheld by `HSTRING`.
    // See <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messageboxw>.
    let result = unsafe {
        // Display a pop-up for users.
        WindowsAndMessaging::MessageBoxW(
            // No owner window.
            None,
            // Set the body text to the given message.
            &windows::core::HSTRING::from(message),
            // Use the default error title.
            None,
            // Display an error icon and provide an 'okay' button.
            WindowsAndMessaging::MB_ICONERROR | WindowsAndMessaging::MB_OK,
        )
    };

    if result == MESSAGE_BOX_ERROR {
        // SAFETY: Microsoft's documentation does not list any safety requirements. See
        // <https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-getlasterror>.
        let error = unsafe { Foundation::GetLastError() };

        panic!("failed with code {:#x} to open up window to display error '{message}'", error.0);
    }
}
