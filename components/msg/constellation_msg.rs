/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The high-level interface from script to constellation. Using this abstract interface helps
//! reduce coupling between these two components.

use nonzero::NonZeroU32;
use std::cell::Cell;
use std::fmt;
use webrender_api;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeyState {
    Pressed,
    Released,
    Repeated,
}

//N.B. Based on the glutin key enum
#[derive(Clone, Copy, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize)]
pub enum Key {
    Space,
    Apostrophe,
    Comma,
    Minus,
    Period,
    Slash,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Semicolon,
    Equal,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    LeftBracket,
    Backslash,
    RightBracket,
    GraveAccent,
    World1,
    World2,

    Escape,
    Enter,
    Tab,
    Backspace,
    Insert,
    Delete,
    Right,
    Left,
    Down,
    Up,
    PageUp,
    PageDown,
    Home,
    End,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpDecimal,
    KpDivide,
    KpMultiply,
    KpSubtract,
    KpAdd,
    KpEnter,
    KpEqual,
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftSuper,
    RightShift,
    RightControl,
    RightAlt,
    RightSuper,
    Menu,

    NavigateBackward,
    NavigateForward,
}

bitflags! {
    #[derive(Deserialize, Serialize)]
    pub struct KeyModifiers: u8 {
        const NONE = 0x00;
        const SHIFT = 0x01;
        const CONTROL = 0x02;
        const ALT = 0x04;
        const SUPER = 0x08;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum TraversalDirection {
    Forward(usize),
    Back(usize),
}

/// Each pipeline ID needs to be unique. However, it also needs to be possible to
/// generate the pipeline ID from an iframe element (this simplifies a lot of other
/// code that makes use of pipeline IDs).
///
/// To achieve this, each pipeline index belongs to a particular namespace. There is
/// a namespace for the constellation thread, and also one for every script thread.
/// This allows pipeline IDs to be generated by any of those threads without conflicting
/// with pipeline IDs created by other script threads or the constellation. The
/// constellation is the only code that is responsible for creating new *namespaces*.
/// This ensures that namespaces are always unique, even when using multi-process mode.
///
/// It may help conceptually to think of the namespace ID as an identifier for the
/// thread that created this pipeline ID - however this is really an implementation
/// detail so shouldn't be relied upon in code logic. It's best to think of the
/// pipeline ID as a simple unique identifier that doesn't convey any more information.
#[derive(Clone, Copy)]
pub struct PipelineNamespace {
    id: PipelineNamespaceId,
    index: u32,
}

impl PipelineNamespace {
    pub fn install(namespace_id: PipelineNamespaceId) {
        PIPELINE_NAMESPACE.with(|tls| {
            assert!(tls.get().is_none());
            tls.set(Some(PipelineNamespace {
                id: namespace_id,
                index: 0,
            }));
        });
    }

    fn next_index(&mut self) -> NonZeroU32 {
        self.index += 1;
        NonZeroU32::new(self.index).expect("pipeline id index wrapped!")
    }

    fn next_pipeline_id(&mut self) -> PipelineId {
        PipelineId {
            namespace_id: self.id,
            index: PipelineIndex(self.next_index()),
        }
    }

    fn next_browsing_context_id(&mut self) -> BrowsingContextId {
        BrowsingContextId {
            namespace_id: self.id,
            index: BrowsingContextIndex(self.next_index()),
        }
    }

    fn next_history_state_id(&mut self) -> HistoryStateId {
        HistoryStateId {
            namespace_id: self.id,
            index: HistoryStateIndex(self.next_index()),
        }
    }
}

thread_local!(pub static PIPELINE_NAMESPACE: Cell<Option<PipelineNamespace>> = Cell::new(None));

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, MallocSizeOf, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PipelineNamespaceId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PipelineIndex(pub NonZeroU32);
malloc_size_of_is_0!(PipelineIndex);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, MallocSizeOf, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PipelineId {
    pub namespace_id: PipelineNamespaceId,
    pub index: PipelineIndex
}

impl PipelineId {
    pub fn new() -> PipelineId {
        PIPELINE_NAMESPACE.with(|tls| {
            let mut namespace = tls.get().expect("No namespace set for this thread!");
            let new_pipeline_id = namespace.next_pipeline_id();
            tls.set(Some(namespace));
            new_pipeline_id
        })
    }

    pub fn to_webrender(&self) -> webrender_api::PipelineId {
        let PipelineNamespaceId(namespace_id) = self.namespace_id;
        let PipelineIndex(index) = self.index;
        webrender_api::PipelineId(namespace_id, index.get())
    }

    #[allow(unsafe_code)]
    pub fn from_webrender(pipeline: webrender_api::PipelineId) -> PipelineId {
        let webrender_api::PipelineId(namespace_id, index) = pipeline;
        unsafe {
            PipelineId {
                namespace_id: PipelineNamespaceId(namespace_id),
                index: PipelineIndex(NonZeroU32::new_unchecked(index)),
            }
        }
    }

    pub fn root_scroll_node(&self) -> webrender_api::ClipId {
        webrender_api::ClipId::root_scroll_node(self.to_webrender())
    }

    pub fn root_scroll_id(&self) -> webrender_api::ExternalScrollId {
        webrender_api::ExternalScrollId(0, self.to_webrender())
    }

    pub fn root_clip_and_scroll_info(&self) -> webrender_api::ClipAndScrollInfo {
        webrender_api::ClipAndScrollInfo::simple(self.root_scroll_node())
    }
}

impl fmt::Display for PipelineId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let PipelineNamespaceId(namespace_id) = self.namespace_id;
        let PipelineIndex(index) = self.index;
        write!(fmt, "({},{})", namespace_id, index.get())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BrowsingContextIndex(pub NonZeroU32);
malloc_size_of_is_0!(BrowsingContextIndex);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, MallocSizeOf, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BrowsingContextId {
    pub namespace_id: PipelineNamespaceId,
    pub index: BrowsingContextIndex,
}

impl BrowsingContextId {
    pub fn new() -> BrowsingContextId {
        PIPELINE_NAMESPACE.with(|tls| {
            let mut namespace = tls.get().expect("No namespace set for this thread!");
            let new_browsing_context_id = namespace.next_browsing_context_id();
            tls.set(Some(namespace));
            new_browsing_context_id
        })
    }
}

impl fmt::Display for BrowsingContextId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let PipelineNamespaceId(namespace_id) = self.namespace_id;
        let BrowsingContextIndex(index) = self.index;
        write!(fmt, "({},{})", namespace_id, index.get())
    }
}

thread_local!(pub static TOP_LEVEL_BROWSING_CONTEXT_ID: Cell<Option<TopLevelBrowsingContextId>> = Cell::new(None));

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, MallocSizeOf, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TopLevelBrowsingContextId(BrowsingContextId);

impl TopLevelBrowsingContextId {
    pub fn new() -> TopLevelBrowsingContextId {
        TopLevelBrowsingContextId(BrowsingContextId::new())
    }
    /// Each script and layout thread should have the top-level browsing context id installed,
    /// since it is used by crash reporting.
    pub fn install(id: TopLevelBrowsingContextId) {
        TOP_LEVEL_BROWSING_CONTEXT_ID.with(|tls| tls.set(Some(id)))
    }

    pub fn installed() -> Option<TopLevelBrowsingContextId> {
        TOP_LEVEL_BROWSING_CONTEXT_ID.with(|tls| tls.get())
    }
}

impl fmt::Display for TopLevelBrowsingContextId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl From<TopLevelBrowsingContextId> for BrowsingContextId {
    fn from(id: TopLevelBrowsingContextId) -> BrowsingContextId {
        id.0
    }
}

impl PartialEq<TopLevelBrowsingContextId> for BrowsingContextId {
    fn eq(&self, rhs: &TopLevelBrowsingContextId) -> bool {
        self.eq(&rhs.0)
    }
}

impl PartialEq<BrowsingContextId> for TopLevelBrowsingContextId {
    fn eq(&self, rhs: &BrowsingContextId) -> bool {
        self.0.eq(rhs)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HistoryStateIndex(pub NonZeroU32);
malloc_size_of_is_0!(HistoryStateIndex);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, MallocSizeOf, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HistoryStateId {
    pub namespace_id: PipelineNamespaceId,
    pub index: HistoryStateIndex,
}

impl HistoryStateId {
    pub fn new() -> HistoryStateId {
        PIPELINE_NAMESPACE.with(|tls| {
            let mut namespace = tls.get().expect("No namespace set for this thread!");
            let next_history_state_id = namespace.next_history_state_id();
            tls.set(Some(namespace));
            next_history_state_id
        })
    }
}

impl fmt::Display for HistoryStateId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let PipelineNamespaceId(namespace_id) = self.namespace_id;
        let HistoryStateIndex(index) = self.index;
        write!(fmt, "({},{})", namespace_id, index.get())
    }
}

// We provide ids just for unit testing.
pub const TEST_NAMESPACE: PipelineNamespaceId = PipelineNamespaceId(1234);
#[allow(unsafe_code)]
#[cfg(feature = "unstable")]
pub const TEST_PIPELINE_INDEX: PipelineIndex = unsafe { PipelineIndex(NonZeroU32::new_unchecked(5678)) };
#[cfg(feature = "unstable")]
pub const TEST_PIPELINE_ID: PipelineId = PipelineId { namespace_id: TEST_NAMESPACE, index: TEST_PIPELINE_INDEX };
#[allow(unsafe_code)]
#[cfg(feature = "unstable")]
pub const TEST_BROWSING_CONTEXT_INDEX: BrowsingContextIndex =
    unsafe { BrowsingContextIndex(NonZeroU32::new_unchecked(8765)) };
#[cfg(feature = "unstable")]
pub const TEST_BROWSING_CONTEXT_ID: BrowsingContextId =
    BrowsingContextId { namespace_id: TEST_NAMESPACE, index: TEST_BROWSING_CONTEXT_INDEX };

// Used to specify the kind of input method editor appropriate to edit a field.
// This is a subset of htmlinputelement::InputType because some variants of InputType
// don't make sense in this context.
#[derive(Deserialize, Serialize)]
pub enum InputMethodType {
    Color,
    Date,
    DatetimeLocal,
    Email,
    Month,
    Number,
    Password,
    Search,
    Tel,
    Text,
    Time,
    Url,
    Week,
}
