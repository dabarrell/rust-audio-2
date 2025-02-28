use wasm_bindgen::JsCast;
use wasm_bindgen::{prelude::Closure, JsValue};
use web_sys::File;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub async fn read_file_to_array_buffer(file: File) -> Result<js_sys::ArrayBuffer, JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let file_reader = web_sys::FileReader::new().unwrap();

        // Set up onload handler
        let file_reader_clone = file_reader.clone();
        let onload_cb = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let array_buffer = file_reader_clone.result().unwrap();
            resolve.call1(&JsValue::NULL, &array_buffer).unwrap();
        }) as Box<dyn FnMut(web_sys::Event)>);

        // Set up onerror handler
        let onerror_cb = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            reject.call0(&JsValue::NULL).unwrap();
        }) as Box<dyn FnMut(web_sys::Event)>);

        file_reader.set_onload(Some(onload_cb.as_ref().unchecked_ref()));
        file_reader.set_onerror(Some(onerror_cb.as_ref().unchecked_ref()));

        // Read the file as an ArrayBuffer
        file_reader.read_as_array_buffer(&file).unwrap();

        // Forget the closures to keep them alive
        onload_cb.forget();
        onerror_cb.forget();
    });

    // Convert the Promise to a Future and await it
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(result.into())
}
