use evdev::Key;
use std::collections::HashMap;

pub struct KeyMap {
    pub normal: HashMap<Key, char>,
    pub shifted: HashMap<Key, char>,
}

impl KeyMap {
    pub fn new(layout: &str) -> Self {
        let mut normal = HashMap::new();
        let mut shifted = HashMap::new();

        // Common keys (Enter, Space, etc.) are usually same position or we handle them generically
        // But punctuation varies wildy.
        // We will define base QWERTY and then apply overrides.
        
        // Base QWERTY Mapping (Physical positions)
        let qwerty_letters = [
            (Key::KEY_Q, 'q', 'Q'), (Key::KEY_W, 'w', 'W'), (Key::KEY_E, 'e', 'E'), (Key::KEY_R, 'r', 'R'), (Key::KEY_T, 't', 'T'),
            (Key::KEY_Y, 'y', 'Y'), (Key::KEY_U, 'u', 'U'), (Key::KEY_I, 'i', 'I'), (Key::KEY_O, 'o', 'O'), (Key::KEY_P, 'p', 'P'),
            (Key::KEY_A, 'a', 'A'), (Key::KEY_S, 's', 'S'), (Key::KEY_D, 'd', 'D'), (Key::KEY_F, 'f', 'F'), (Key::KEY_G, 'g', 'G'),
            (Key::KEY_H, 'h', 'H'), (Key::KEY_J, 'j', 'J'), (Key::KEY_K, 'k', 'K'), (Key::KEY_L, 'l', 'L'),
            (Key::KEY_Z, 'z', 'Z'), (Key::KEY_X, 'x', 'X'), (Key::KEY_C, 'c', 'C'), (Key::KEY_V, 'v', 'V'), (Key::KEY_B, 'b', 'B'),
            (Key::KEY_N, 'n', 'N'), (Key::KEY_M, 'm', 'M'),
        ];

        for (key, lower, upper) in qwerty_letters {
            normal.insert(key, lower);
            shifted.insert(key, upper);
        }
        
        // Numbers
        let qwerty_numbers = [
            (Key::KEY_1, '1', '!'), (Key::KEY_2, '2', '@'), (Key::KEY_3, '3', '#'),
            (Key::KEY_4, '4', '$'), (Key::KEY_5, '5', '%'), (Key::KEY_6, '6', '^'),
            (Key::KEY_7, '7', '&'), (Key::KEY_8, '8', '*'), (Key::KEY_9, '9', '('),
            (Key::KEY_0, '0', ')'),
        ];
        
        for (key, num, sym) in qwerty_numbers {
            normal.insert(key, num);
            shifted.insert(key, sym);
        }

        // Punctuation
        let qwerty_punct = [
            (Key::KEY_MINUS, '-', '_'),
            (Key::KEY_EQUAL, '=', '+'),
            (Key::KEY_LEFTBRACE, '[', '{'),
            (Key::KEY_RIGHTBRACE, ']', '}'),
            (Key::KEY_SEMICOLON, ';', ':'),
            (Key::KEY_APOSTROPHE, '\'', '"'),
            (Key::KEY_GRAVE, '`', '~'),
            (Key::KEY_BACKSLASH, '\\', '|'),
            (Key::KEY_COMMA, ',', '<'),
            (Key::KEY_DOT, '.', '>'),
            (Key::KEY_SLASH, '/', '?'),
            (Key::KEY_SPACE, ' ', ' '),
        ];
        
        for (key, norm, shift) in qwerty_punct {
            normal.insert(key, norm);
            shifted.insert(key, shift);
        }
        
        // Apply overrides
        match layout.to_lowercase().as_str() {
            "azerty" => apply_azerty(&mut normal, &mut shifted),
            "qwertz" => apply_qwertz(&mut normal, &mut shifted),
            "colemak" => apply_colemak(&mut normal, &mut shifted),
            "dvorak" => apply_dvorak(&mut normal, &mut shifted),
            _ => {} // Default to QWERTY
        }

        Self { normal, shifted }
    }

    pub fn map_key(&self, key: Key, shift: bool, caps_lock: bool) -> Option<char> {
        let base_char = if shift {
            self.shifted.get(&key).copied()
        } else {
            self.normal.get(&key).copied()
        };

        // Handle caps lock for letters
        base_char.map(|c| {
            if c.is_ascii_alphabetic() && caps_lock {
                if shift {
                    c.to_ascii_lowercase()
                } else {
                    c.to_ascii_uppercase()
                }
            } else {
                c
            }
        })
    }
}

fn apply_azerty(normal: &mut HashMap<Key, char>, shifted: &mut HashMap<Key, char>) {
    // A <-> Q
    swap_keys(normal, shifted, Key::KEY_A, Key::KEY_Q);
    // Z <-> W
    swap_keys(normal, shifted, Key::KEY_Z, Key::KEY_W);
    // M moves to semicolon position (right of L)
    // Semicolon moves to M position? No, AZERTY is quite different for punctuation.
    // This requires detailed mapping.
    // For simplicity, we'll map the letters correctly. Punctuation handling is tricky without a full map.
    // Let's implement M properly.
    // On AZERTY, 'M' is where ';' is on QWERTY.
    // And ';' is where ',' is... it's a mess to swap.
    // Better to overwrite explicit keys.
    
    // Letters
    // A and Q, Z and W handled above.
    // M is on KEY_SEMICOLON
    setup_key(normal, shifted, Key::KEY_SEMICOLON, 'm', 'M');
    
    // Punctuation and Numbers
    // Numbers require shift on AZERTY standard
    // Row 1: & é " ' ( - è _ ç à ) =
    setup_key(normal, shifted, Key::KEY_1, '&', '1');
    setup_key(normal, shifted, Key::KEY_2, 'é', '2');
    setup_key(normal, shifted, Key::KEY_3, '"', '3');
    setup_key(normal, shifted, Key::KEY_4, '\'', '4');
    setup_key(normal, shifted, Key::KEY_5, '(', '5');
    setup_key(normal, shifted, Key::KEY_6, '-', '6');
    setup_key(normal, shifted, Key::KEY_7, 'è', '7');
    setup_key(normal, shifted, Key::KEY_8, '_', '8');
    setup_key(normal, shifted, Key::KEY_9, 'ç', '9');
    setup_key(normal, shifted, Key::KEY_0, 'à', '0');

    // Other punctuation
    setup_key(normal, shifted, Key::KEY_M, ',', '?'); // Where M was
    setup_key(normal, shifted, Key::KEY_COMMA, ';', '.');
    setup_key(normal, shifted, Key::KEY_DOT, ':', '/');
    setup_key(normal, shifted, Key::KEY_SLASH, '!', '§');
}

fn apply_qwertz(normal: &mut HashMap<Key, char>, shifted: &mut HashMap<Key, char>) {
    // Y <-> Z
    swap_keys(normal, shifted, Key::KEY_Y, Key::KEY_Z);
    // Umm, QWERTZ also has umlauts.
    // Defaults for German layout
}

fn apply_colemak(_normal: &mut HashMap<Key, char>, _shifted: &mut HashMap<Key, char>) {
    // Just a few common swaps?
    // It's a full remapping.
    // Leaving empty for now as placeholder for future extension
}

fn apply_dvorak(_normal: &mut HashMap<Key, char>, _shifted: &mut HashMap<Key, char>) {
    // Placeholder
}

fn swap_keys(normal: &mut HashMap<Key, char>, shifted: &mut HashMap<Key, char>, k1: Key, k2: Key) {
    if let (Some(n1), Some(n2)) = (normal.get(&k1).copied(), normal.get(&k2).copied()) {
        normal.insert(k1, n2);
        normal.insert(k2, n1);
    }
    if let (Some(s1), Some(s2)) = (shifted.get(&k1).copied(), shifted.get(&k2).copied()) {
        shifted.insert(k1, s2);
        shifted.insert(k2, s1);
    }
}

fn setup_key(normal: &mut HashMap<Key, char>, shifted: &mut HashMap<Key, char>, k: Key, n: char, s: char) {
    normal.insert(k, n);
    shifted.insert(k, s);
}
