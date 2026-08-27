use tokmesh_core::ClientId;

pub struct ClientUi {
    pub display_name: &'static str,
    pub hotkey: char,
}

pub const CLIENT_UI: [ClientUi; ClientId::COUNT] = [
    ClientUi {
        display_name: "OpenCode",
        hotkey: '1',
    },
    ClientUi {
        display_name: "Claude",
        hotkey: '2',
    },
    ClientUi {
        display_name: "Codex",
        hotkey: '3',
    },
    ClientUi {
        display_name: "Cursor",
        hotkey: '4',
    },
    ClientUi {
        display_name: "Gemini",
        hotkey: '5',
    },
    ClientUi {
        display_name: "Amp",
        hotkey: '6',
    },
    ClientUi {
        display_name: "Droid",
        hotkey: '7',
    },
    ClientUi {
        display_name: "OpenClaw",
        hotkey: '8',
    },
    ClientUi {
        display_name: "Pi",
        hotkey: '9',
    },
    ClientUi {
        display_name: "Kimi",
        hotkey: '0',
    },
    ClientUi {
        display_name: "Qwen",
        hotkey: 'w',
    },
    ClientUi {
        display_name: "Roo Code",
        hotkey: 'r',
    },
    ClientUi {
        display_name: "KiloCode",
        hotkey: 'k',
    },
    ClientUi {
        display_name: "Mux",
        hotkey: 'x',
    },
    ClientUi {
        display_name: "Kilo CLI",
        hotkey: 'l',
    },
    ClientUi {
        display_name: "Crush",
        hotkey: 'h',
    },
    ClientUi {
        display_name: "Hermes Agent",
        hotkey: 'e',
    },
    ClientUi {
        display_name: "Copilot",
        hotkey: 'c',
    },
    ClientUi {
        display_name: "Goose",
        hotkey: 'o',
    },
    ClientUi {
        display_name: "Codebuff",
        hotkey: 'b',
    },
    ClientUi {
        display_name: "Antigravity",
        hotkey: 'a',
    },
    ClientUi {
        display_name: "Zed Agent",
        hotkey: 'z',
    },
    ClientUi {
        display_name: "Kiro",
        hotkey: 'i',
    },
    ClientUi {
        display_name: "Trae",
        hotkey: 'y',
    },
    ClientUi {
        display_name: "Warp",
        hotkey: 'v',
    },
    ClientUi {
        display_name: "Cline",
        hotkey: 'n',
    },
    ClientUi {
        display_name: "Gajae-Code",
        hotkey: 'g',
    },
    ClientUi {
        display_name: "Grok Build",
        hotkey: 'u',
    },
    ClientUi {
        display_name: "Jcode",
        hotkey: 'j',
    },
    ClientUi {
        display_name: "Command Code",
        hotkey: 'd',
    },
    ClientUi {
        display_name: "MiMo Code",
        hotkey: 'm',
    },
    ClientUi {
        display_name: "Antigravity CLI",
        hotkey: 'f',
    },
    ClientUi {
        display_name: "Junie",
        hotkey: 'p',
    },
    ClientUi {
        display_name: "ZCode",
        hotkey: 'q',
    },
    ClientUi {
        display_name: "OpenCodeReview",
        hotkey: 'O',
    },
    ClientUi {
        display_name: "CodeBuddy",
        hotkey: 'C',
    },
    ClientUi {
        display_name: "WorkBuddy",
        hotkey: 'B',
    },
    ClientUi {
        display_name: "Devin CLI",
        hotkey: 'D',
    },
    ClientUi {
        display_name: "Devin Desktop",
        hotkey: 'E',
    },
    ClientUi {
        display_name: "Senpi (OmO Native)",
        hotkey: 'S',
    },
    ClientUi {
        display_name: "Augment Code",
        hotkey: 'A',
    },
    ClientUi {
        display_name: "Kimchi",
        hotkey: 'K',
    },
    ClientUi {
        display_name: "Reasonix",
        hotkey: 'R',
    },
    ClientUi {
        display_name: "Prime Agent",
        hotkey: 'P',
    },
    ClientUi {
        display_name: "Freebuff",
        hotkey: 'F',
    },
    ClientUi {
        display_name: "Cherry Studio",
        hotkey: 'G',
    },
    ClientUi {
        display_name: "DeepSeek Harness",
        hotkey: 't',
    },
    ClientUi {
        display_name: "MiniMax Code",
        hotkey: 'M',
    },
    ClientUi {
        display_name: "Fx",
        hotkey: 'X',
    },
    ClientUi {
        display_name: "Oh My Pi",
        hotkey: 'Y',
    },
];

pub fn display_name(client: ClientId) -> &'static str {
    CLIENT_UI[client as usize].display_name
}

/// Compact label for constrained TUI columns. Product-facing surfaces should
/// use [`display_name`] so the canonical registry label is preserved.
pub fn compact_display_name(client: ClientId) -> &'static str {
    match client {
        ClientId::Senpi => "Senpi",
        // "DeepSeek Harness" (16 cells) overflows the 15-cell Client column.
        ClientId::Dsh => "DeepSeek",
        _ => display_name(client),
    }
}

pub fn hotkey(client: ClientId) -> char {
    CLIENT_UI[client as usize].hotkey
}

pub fn from_hotkey(key: char) -> Option<ClientId> {
    CLIENT_UI.iter().enumerate().find_map(|(i, ui)| {
        if ui.hotkey == key {
            ClientId::ALL.get(i).copied()
        } else {
            None
        }
    })
}
