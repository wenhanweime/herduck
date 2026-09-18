//! Host-only sidebar art. This cache is per client and never enters an agent PTY.
use ratatui::layout::Rect;

use super::{encode_delete_image, encode_display_placement, encode_kitty_data, ClippedPlacement};

// Terminal images start at 10_000; pane-layer images use the high bit.
const IMAGE_ID: u32 = 9_999;
const PNG: &[u8] = include_bytes!("../../assets/brand/herduck-kitty-32-outline.png");

#[derive(Debug, Default, Clone)]
pub(super) struct BrandGraphicsCache {
    placement: Option<Rect>,
}

impl BrandGraphicsCache {
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.placement.is_none()
    }

    pub(super) fn encode(&mut self, out: &mut Vec<u8>, rect: Option<Rect>) {
        if rect == self.placement {
            return;
        }
        // Delete the old placement and data together when moving or hiding it.
        if self.placement.take().is_some() {
            encode_delete_image(out, IMAGE_ID);
        }
        let Some(rect) = rect else { return };
        encode_kitty_data(out, &format!("a=t,t=d,f=100,i={IMAGE_ID},q=2"), PNG);
        encode_display_placement(
            out,
            ClippedPlacement {
                x: rect.x,
                y: rect.y,
                cols: rect.width.into(),
                rows: rect.height.into(),
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                x_offset: 0,
                y_offset: 0,
            },
            IMAGE_ID,
            1,
            0,
        );
        self.placement = Some(rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn logo_lifecycle_handles_views_overlays_resize_and_independent_clients() {
        use crate::app::state::{AppState, SidebarView};
        use crate::app::Mode;
        use crate::kitty_graphics::{encode_local_pane_graphics, HostCellSize, HostGraphicsCache};
        use crate::terminal::TerminalRuntimeRegistry;
        let mut app = AppState::test_new();
        app.mode = Mode::Terminal;
        app.kitty_graphics_enabled = true;
        app.sidebar_width = 26;
        let runtimes = TerminalRuntimeRegistry::new();
        let cell = HostCellSize {
            width_px: 8,
            height_px: 16,
        };
        let mut first = HostGraphicsCache::default();
        let mut second = HostGraphicsCache::default();
        crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
        for cache in [&mut first, &mut second] {
            let bytes = encode_local_pane_graphics(&app, &runtimes, cell, cache);
            assert!(String::from_utf8(bytes).unwrap().contains("f=100,i=9999"));
            assert!(!cache.is_empty());
        }
        // Full-screen settings cover the footer; the launcher menu does not.
        app.mode = Mode::Settings;
        assert_eq!(
            encode_local_pane_graphics(&app, &runtimes, cell, &mut first),
            b"\x1b_Ga=d,d=I,i=9999,q=2;\x1b\\"
        );
        assert!(first.is_empty());
        assert!(!second.is_empty());
        app.mode = Mode::Navigate;
        for view in [
            SidebarView::SpacesAgents,
            SidebarView::Sessions,
            SidebarView::Projects,
            SidebarView::Clusters,
        ] {
            app.sidebar_view = view;
            crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
            encode_local_pane_graphics(&app, &runtimes, cell, &mut first);
            assert!(!first.is_empty());
            let logo = crate::ui::sidebar_logo_rect(&app).unwrap();
            app.mode = Mode::GlobalMenu;
            crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
            assert_eq!(crate::ui::sidebar_logo_rect(&app), Some(logo));
            assert!(!app.global_menu_rect().intersects(logo));
            first.test_mark_non_empty();
            let bytes = encode_local_pane_graphics(&app, &runtimes, cell, &mut first);
            assert!(!bytes.is_empty(), "menu still clears pane images");
            assert!(!first.has_pane_graphics());
            assert!(!first.brand.is_empty(), "menu retains the sidebar logo");
            assert!(
                !String::from_utf8(bytes).unwrap().contains("i=9999"),
                "opening the menu must not delete or re-upload the logo"
            );
            let mut fresh = HostGraphicsCache::default();
            let bytes = encode_local_pane_graphics(&app, &runtimes, cell, &mut fresh);
            assert!(
                String::from_utf8(bytes).unwrap().contains("f=100,i=9999"),
                "a client connecting with the menu open still receives the logo"
            );
            app.mode = Mode::Navigate;
            crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
            assert!(encode_local_pane_graphics(&app, &runtimes, cell, &mut first).is_empty());
        }
        first.test_mark_non_empty();
        let bytes = encode_local_pane_graphics(&app, &runtimes, cell, &mut first);
        assert!(!bytes.is_empty(), "navigation clears pane graphics");
        assert!(!first.has_pane_graphics());
        assert!(
            !first.brand.is_empty(),
            "navigation retains the sidebar logo"
        );
        assert!(encode_local_pane_graphics(&app, &runtimes, cell, &mut first).is_empty());
        app.sidebar_collapsed = true;
        crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
        assert!(!encode_local_pane_graphics(&app, &runtimes, cell, &mut first).is_empty());
        assert!(first.is_empty());
        app.sidebar_collapsed = false;
        crate::ui::compute_view(&mut app, Rect::new(0, 0, 110, 32));
        encode_local_pane_graphics(&app, &runtimes, cell, &mut first);
        crate::ui::compute_view(&mut app, Rect::new(0, 0, 40, 20));
        encode_local_pane_graphics(&app, &runtimes, cell, &mut first);
        assert!(first.is_empty(), "mobile layout clears sidebar image");
        assert!(
            !encode_local_pane_graphics(&app, &runtimes, HostCellSize::default(), &mut second)
                .is_empty()
        );
        assert!(second.is_empty(), "disabled client clears its own image");
    }

    #[test]
    fn logo_transmits_exact_32px_png_then_caches_moves_and_cleans_up() {
        let decoder = png::Decoder::new(std::io::Cursor::new(PNG));
        let reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (32, 32));
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        let mut cache = BrandGraphicsCache::default();
        let mut bytes = Vec::new();
        let rect = Rect::new(2, 20, 4, 2);
        cache.encode(&mut bytes, Some(rect));
        let output = String::from_utf8(bytes.clone()).unwrap();
        let encoded = output
            .split_once(';')
            .unwrap()
            .1
            .split_once("\x1b\\")
            .unwrap()
            .0;
        assert!(encoded.len() <= 4096);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
            PNG
        );
        assert!(output.contains("\x1b[21;3H\x1b_Ga=p,i=9999,p=1,c=4,r=2"));
        bytes.clear();
        cache.encode(&mut bytes, Some(rect));
        assert!(bytes.is_empty(), "no duplicate upload on unchanged frame");
        cache.encode(&mut bytes, Some(Rect::new(2, 30, 4, 2)));
        assert!(bytes.starts_with(b"\x1b_Ga=d,d=I,i=9999,q=2;\x1b\\"));
        bytes.clear();
        cache.encode(&mut bytes, None);
        assert_eq!(bytes, b"\x1b_Ga=d,d=I,i=9999,q=2;\x1b\\");
        assert!(cache.is_empty());
    }
}
