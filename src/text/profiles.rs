//! Verified font layouts. Only tile locations and labels are stored here.
use super::{TextRenderer, tile_pixels};

impl TextRenderer {
    pub(super) fn learn_profile(&mut self, chr: &[u8]) {
        // Only semantic tile locations are stored here, never game graphics.
        // This CHR fingerprint identifies the verified SMB3 European font layout.
        let fingerprint = chr.iter().fold(0xcbf29ce484222325u64, |hash, &b| {
            (hash ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        if chr.len() == 131072 && fingerprint == 0xf921ce5b9b1d4cd2 {
            for (index, c) in [
                (0x1e0a, 'P'),
                (0x1e0b, 'L'),
                (0x1e0c, 'A'),
                (0x1e0d, 'R'),
                (0x1e1a, 'Y'),
                (0x1e1b, 'E'),
                (0x1e1c, 'G'),
                (0x1e1d, 'M'),
                (0x1e3f, '©'),
            ] {
                self.add_tile(c, &tile_pixels(&chr[index * 16..(index + 1) * 16]), 4, 4);
            }
            for (base, subset, chars) in [
                (0x17b0, 1, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
                (0x17f0, 4, "0123456789"),
                (0x1f76, 2, "0123456789"),
                (0x05f0, 4, "0123456789"),
            ] {
                for (offset, c) in chars.chars().enumerate() {
                    let index = base + offset;
                    self.add_tile(
                        c,
                        &tile_pixels(&chr[index * 16..(index + 1) * 16]),
                        subset,
                        4,
                    );
                }
            }
            let super_tiles: Vec<usize> = (0..4)
                .flat_map(|row| (0..10).map(move |col| 0x1e30 + row * 16 + col))
                .collect();
            self.add_word(chr, "SUPER", 10, &super_tiles, 4);
            self.add_word(
                chr,
                "MARIO BROS.",
                22,
                &[
                    0x1e70, 0x1e71, 0x1e72, 0x1e73, 0x1e74, 0x1e75, 0x1e76, 0x1e77, 0x1e32, 0x1e3a,
                    0x1e3a, 0x1e3a, 0x1e3a, 0x1e7d, 0x1e7e, 0x1e7f, 0x1ec0, 0x1ec1, 0x1e3a, 0x1ec3,
                    0x1ec4, 0x1e3a, 0x1e80, 0x1e5d, 0x1e82, 0x1e83, 0x1e84, 0x1e85, 0x1e86, 0x1e87,
                    0x1e88, 0x1e89, 0x1e8a, 0x1e8b, 0x1e3a, 0x1e8d, 0x1e8e, 0x1e8f, 0x1ed0, 0x1ed1,
                    0x1ed2, 0x1ed3, 0x1ed4, 0x1ed5, 0x1e90, 0x1e91, 0x1e92, 0x1e93, 0x1e94, 0x1e95,
                    0x1e96, 0x1e97, 0x1e98, 0x1e99, 0x1e9a, 0x1e9b, 0x1e9c, 0x1e9d, 0x1e9e, 0x1e9f,
                    0x1ee0, 0x1ee1, 0x1ee2, 0x1ee3, 0x1ee4, 0x1ee5, 0x1ea0, 0x1ea1, 0x1ea2, 0x1ea3,
                    0x1ea4, 0x1ea5, 0x1ea6, 0x1ea7, 0x1ea8, 0x1ea9, 0x1eaa, 0x1eab, 0x1eac, 0x1ead,
                    0x1eae, 0x1eaf, 0x1ef0, 0x1ef1, 0x1ef2, 0x1ef3, 0x1ef4, 0x1ef5, 0x1e3a, 0x1eb1,
                    0x1eb2, 0x1eb3, 0x1eb4, 0x1eb5, 0x1eb6, 0x1eb7, 0x1eb8, 0x1eb9, 0x1eba, 0x1ebb,
                    0x1ebc, 0x1e3a, 0x1e5e, 0x1ebf, 0x1ec6, 0x1ec7, 0x1ec8, 0x1ed6, 0x1ed7, 0x1ed8,
                ],
                4,
            );
            self.add_word(
                chr,
                "3",
                5,
                &[
                    0x1ec9, 0x1eca, 0x1e8a, 0x1ecc, 0x1e3a, 0x1ed9, 0x1eda, 0x1edb, 0x1edc, 0x1edd,
                    0x1ee9, 0x1eea, 0x1eeb, 0x1eec, 0x1edf, 0x1ef9, 0x1efa, 0x1efb, 0x1eff, 0x1efd,
                    0x1e6c, 0x1e6d, 0x1e6e, 0x1e6f, 0x1ede,
                ],
                4,
            );
            self.add_word(chr, "WORLD", 4, &[0x1770, 0x1771, 0x1772, 0x1773], 4);
            self.add_word(
                chr,
                "Nintendo",
                6,
                &[0x1e2a, 0x1e2b, 0x1e2c, 0x1e2d, 0x1e2e, 0x1e2f],
                2,
            );
        }
    }
}
