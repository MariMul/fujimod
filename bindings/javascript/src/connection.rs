use super::*;
use js_sys::Array;

#[wasm_bindgen]
#[derive(Clone)]
pub struct Device {
    pub(crate) inner: usb::connection::Device,
    pub(crate) proxy: Option<std::sync::Arc<fujimod_usb::remote::Observation>>,
}
impl Device {
    fn owner(&self) -> Result<&std::sync::Arc<fujimod_usb::owner::DeviceOwner>, JsValue> {
        match &self.inner {
            usb::connection::Device::Owned(owner) => Ok(owner),
            _ => Err(fail(
                ErrorKind::InvalidArgument,
                "This operation requires an owned browser USB device",
            )),
        }
    }
}
#[wasm_bindgen]
impl Device {
    /// Discovery and requestDevice open the page owner once; repeated open is
    /// idempotent. Reopening after clean shutdown reuses the same physical owner.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn open(&self) -> js_sys::Promise {
        let owner = self.owner().cloned();
        let proxy = self.proxy.clone();
        let remote = if let usb::connection::Device::Proxy(remote) = &self.inner {
            Some(remote.clone())
        } else {
            None
        };
        wasm_bindgen_futures::future_to_promise(async move {
            if let Some(proxy) = proxy {
                proxy.open().map_err(|e| error(e.into()))?;
                if let Some(remote) = remote {
                    crate::exporter::monitor_proxy(remote, proxy);
                }
            } else {
                owner?.reopen().await.map_err(|e| error(e.into()))?;
            }
            Ok(JsValue::UNDEFINED)
        })
    }
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Result<String, JsValue> {
        if let Some(proxy) = &self.proxy {
            return Ok(proxy.status().id);
        }
        Ok(self.owner()?.descriptor.id.clone())
    }
    #[wasm_bindgen(getter, unchecked_return_type = "DeviceState")]
    pub fn status(&self) -> Result<JsValue, JsValue> {
        if let Some(proxy) = &self.proxy {
            return crate::exporter::state_value(&proxy.status());
        }
        crate::exporter::state_value(&self.owner()?.status())
    }
    #[wasm_bindgen(getter)]
    pub fn present(&self) -> Result<bool, JsValue> {
        if let Some(proxy) = &self.proxy {
            return Ok(proxy.status().present);
        }
        Ok(self.owner()?.status().present)
    }
    #[wasm_bindgen(getter)]
    pub fn sharing(&self) -> Result<bool, JsValue> {
        if self.proxy.is_some() {
            return Ok(false);
        }
        Ok(self.owner()?.status().sharing)
    }
    #[wasm_bindgen(getter, js_name=sessionOwner, unchecked_return_type="DeviceState['sessionOwner']")]
    pub fn session_owner(&self) -> Result<JsValue, JsValue> {
        js_sys::Reflect::get(&self.status()?, &JsValue::from_str("sessionOwner"))
    }
    #[wasm_bindgen(js_name=watchLog)]
    pub fn watch_log(&self) -> Result<crate::exporter::LogEvents, JsValue> {
        if let Some(proxy) = &self.proxy {
            return Ok(crate::exporter::LogEvents::from_events(proxy.watch_log()));
        }
        Ok(crate::exporter::LogEvents::new(self.owner()?))
    }
    pub fn watch(&self) -> Result<crate::exporter::DeviceEvents, JsValue> {
        if let Some(proxy) = &self.proxy {
            return Ok(crate::exporter::DeviceEvents::from_receiver(proxy.watch()));
        }
        Ok(crate::exporter::DeviceEvents::new(self.owner()?))
    }
    #[wasm_bindgen(js_name=acquireSession, unchecked_return_type="Promise<Session>")]
    pub fn acquire_session(
        &self,
        interface: &InterfaceSelection,
        limits: &FirmwareLimits,
        options: &TransferOptions,
        tool: Option<String>,
    ) -> js_sys::Promise {
        session::Session::open_named(self, interface, limits, options, tool)
    }
    #[wasm_bindgen(js_name=stopSharing)]
    pub fn stop_sharing(&self) -> Result<(), JsValue> {
        self.owner()?.stop_sharing();
        Ok(())
    }
    pub fn share(&self) -> Result<crate::exporter::Exporter, JsValue> {
        let usb::connection::Device::Owned(owner) = &self.inner else {
            return Err(fail(
                ErrorKind::InvalidArgument,
                "Only an owned local USB device can be exported",
            ));
        };
        owner.share().map_err(|e| error(e.into()))?;
        Ok(crate::exporter::Exporter::from_owners(vec![owner.clone()]))
    }
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn close(&self) -> js_sys::Promise {
        let device = self.inner.clone();
        let proxy = self.proxy.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            if let Some(proxy) = proxy {
                proxy.close();
            }
            if let usb::connection::Device::Owned(owner) = device {
                owner.shutdown().await.map_err(|e| error(e.into()))?;
            }
            Ok(JsValue::UNDEFINED)
        })
    }
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
                let inner = match inner {
                    usb::connection::Device::Local(info) => {
                        usb::connection::Device::Owned(crate::exporter::device_owner(info).await?)
                    }
                    other => other,
                };
                out.push(&JsValue::from(Device { inner, proxy: None }));
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
            let Some(info) = nusb::request_device(&[selector])
                .await
                .map_err(|e| error(e.into()))?
            else {
                return Ok(JsValue::UNDEFINED);
            };
            let owner = crate::exporter::device_owner(info).await?;
            Ok(JsValue::from(Device {
                inner: usb::connection::Device::Owned(owner),
                proxy: None,
            }))
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
            let proxy = if let usb::connection::Device::Proxy(remote) = &inner {
                Some(crate::exporter::proxy_observation(remote, &url, &token))
            } else {
                None
            };
            out.push(&JsValue::from(Device { inner, proxy }));
        }
        Ok(out.into())
    }))
}
