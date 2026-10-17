use super::*;
use js_sys::Array;

#[wasm_bindgen]
#[derive(Clone)]
pub struct Device {
    pub(crate) inner: usb::connection::Device,
}
#[wasm_bindgen]
impl Device {
    #[wasm_bindgen(getter,js_name=vendorId)]
    pub fn vendor_id(&self) -> u16 {
        self.inner.vendor_id()
    }
    #[wasm_bindgen(getter,js_name=productId)]
    pub fn product_id(&self) -> u16 {
        self.inner.product_id()
    }
    #[wasm_bindgen(getter,js_name=manufacturer)]
    pub fn manufacturer(&self) -> Option<String> {
        self.inner.manufacturer_string().map(str::to_owned)
    }
    #[wasm_bindgen(getter,js_name=productName)]
    pub fn product_name(&self) -> Option<String> {
        self.inner.product_string().map(str::to_owned)
    }
    #[wasm_bindgen(getter,js_name=serialNumber)]
    pub fn serial_number(&self) -> Option<String> {
        self.inner.serial_number().map(str::to_owned)
    }
    #[wasm_bindgen(unchecked_return_type = "Promise<InterfaceSelection[]>")]
    pub fn interfaces(&self) -> js_sys::Promise {
        let info = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let items = usb::connection::interfaces(&info).await.map_err(error)?;
            let out = Array::new();
            for inner in items {
                out.push(&JsValue::from(InterfaceSelection { inner }));
            }
            Ok(out.into())
        })
    }
}
#[wasm_bindgen(unchecked_return_type = "Promise<Device[]>")]
pub fn discover(
    #[wasm_bindgen(unchecked_param_type = "DeviceFilter")] filter: JsValue,
) -> Result<js_sys::Promise, JsValue> {
    let filter = DeviceFilter::parse(filter)?;
    Ok({
        wasm_bindgen_futures::future_to_promise(async move {
            let items = usb::connection::discover(filter.into())
                .await
                .map_err(error)?;
            let out = Array::new();
            for inner in items {
                out.push(&JsValue::from(Device { inner }));
            }
            Ok(out.into())
        })
    })
}
/// Call in response to a user action; this is the browser's device picker.
#[wasm_bindgen(js_name=requestDevice,unchecked_return_type="Promise<Device | undefined>")]
pub fn request_device(
    #[wasm_bindgen(unchecked_param_type = "DeviceFilter")] filter: JsValue,
) -> Result<js_sys::Promise, JsValue> {
    let filter = DeviceFilter::parse(filter)?;
    Ok({
        wasm_bindgen_futures::future_to_promise(async move {
            let selector = match filter {
                DeviceFilter::Fujifilm => nusb::DeviceSelector::all().with_vid(0x04cb),
                DeviceFilter::Diagnostic => {
                    nusb::DeviceSelector::all().with_vid_pid(0x04cb, 0xff80)
                }
            };
            Ok(nusb::request_device(&[selector])
                .await
                .map_err(|e| error(e.into()))?
                .map(|inner| {
                    JsValue::from(Device {
                        inner: usb::connection::Device::Local(inner),
                    })
                })
                .unwrap_or(JsValue::UNDEFINED))
        })
    })
}

/// Discover through a relay instead of this browser's USB bus.
#[wasm_bindgen(js_name=discoverProxy, unchecked_return_type="Promise<Device[]>")]
pub fn discover_proxy(
    url: String,
    token: String,
    filter: JsValue,
) -> Result<js_sys::Promise, JsValue> {
    let filter = DeviceFilter::parse(filter)?;
    Ok(wasm_bindgen_futures::future_to_promise(async move {
        let items = usb::connection::discover_proxy(&url, &token, filter.into())
            .await
            .map_err(error)?;
        let out = Array::new();
        for inner in items {
            out.push(&JsValue::from(Device { inner }));
        }
        Ok(out.into())
    }))
}
