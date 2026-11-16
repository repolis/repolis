use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(tag = "type", content = "index")]
pub enum Target {
    Building(usize),
    District(usize),
}

fn dispatch(event_name: &str, payload: &impl Serialize) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = serde_wasm_bindgen::to_value(payload) {
                let init = web_sys::CustomEventInit::new();
                init.set_detail(&val);
                init.set_bubbles(true);
                if let Ok(ev) = web_sys::CustomEvent::new_with_event_init_dict(event_name, &init) {
                    let _ = window.dispatch_event(&ev);
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (event_name, payload);
    }
}

pub fn dispatch_hover(target: Option<Target>) {
    dispatch("repolis:hover", &target);
}

pub fn dispatch_select(target: Option<Target>) {
    dispatch("repolis:select", &target);
}

pub fn dispatch_city() {
    dispatch("repolis:city", &());
}
