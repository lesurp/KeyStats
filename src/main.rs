#[macro_use]
extern crate log;
extern crate env_logger;
extern crate libc;
extern crate time;

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

// Hardcoding because rust does not support const initialization with non-const func
// ie Path::new(APP_DIR).join(foo) does not work :(
const DEVICE_PATH: &str = "/dev/input/by-path/";
//const APP_DIR: &str = "/var/opt/keystats/";
const MAIN_FILE: &str = "/var/opt/keystats/keys";
const TMP_FILE: &str = "/var/opt/keystats/keys.tmp";
const OLD_FILES_DIR: &str = "/var/opt/keystats/previous/";

fn main() {
    env_logger::init();

    match std::fs::create_dir_all(OLD_FILES_DIR) {
        Ok(_) => {}
        Err(e) => println!("Couldn't create dirs :(\n{}", e),
    }
    move_previous_key_file();

    let devices = fs::read_dir(DEVICE_PATH).unwrap();

    let mut keyboard_file = None;
    for device in devices {
        let path = device.unwrap();
        let file_name = path.file_name().into_string().unwrap();
        if file_name.contains("kbd") {
            keyboard_file = Some(fs::File::open(path.path()).unwrap());
        }
    }

    match keyboard_file {
        Some(file) => {
            log_keys(file);
        }
        None => {
            panic!("Couldn't find the keyboard device");
        }
    }
}

enum Type {
    Syn,
    Key,
    Sw,
    Rep,
    Other,
}

fn type_to_enum(type_: &libc::__u16) -> Type {
    match type_ {
        0 => Type::Syn,
        1 => Type::Key,
        4 => Type::Sw,
        20 => Type::Rep,
        _ => Type::Other,
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct PressedCombination {
    shift: bool,
    alt: bool,
    meta: bool,
    ctrl: bool,
    code: libc::__u16,
}

fn log_keys(mut keyboard_file: fs::File) {
    use std::mem;

    const BUFFER_LENGTH: usize = mem::size_of::<libc::input_event>();
    let mut buffer: [u8; BUFFER_LENGTH] = [0; BUFFER_LENGTH];

    let mut pressed_combinations = HashMap::<PressedCombination, u32>::new();
    let mut pressed_combination = PressedCombination {
        shift: false,
        alt: false,
        meta: false,
        ctrl: false,
        code: 0,
    };

    let mut counter = 0;
    debug!("Starting key-logging loop");
    loop {
        let n = keyboard_file.read(&mut buffer).unwrap();
        if n != BUFFER_LENGTH {
            continue;
        }
        unsafe {
            let input_event =
                mem::transmute_copy::<[u8; BUFFER_LENGTH], libc::input_event>(&buffer);
            let type_ = &input_event.type_;
            let code = &input_event.code;
            let value = &input_event.value;

            debug!("type/code/value: {}/{}/{}", type_, code, value);

            match type_to_enum(type_) {
                Type::Key => match value {
                    0 => match code {
                        97 | 29 => pressed_combination.ctrl = false,
                        42 | 54 => pressed_combination.shift = false,
                        56 | 100 => pressed_combination.alt = false,
                        125 | 126 => pressed_combination.meta = false,
                        _ => continue,
                    },
                    1 => {
                        match code {
                            29 | 97 => {
                                pressed_combination.code = 29;
                                pressed_combination.ctrl = true;
                            }
                            42 | 54 => {
                                pressed_combination.code = 42;
                                pressed_combination.shift = true;
                            }
                            56 | 100 => {
                                pressed_combination.code = 56;
                                pressed_combination.alt = true;
                            }

                            125 | 126 => {
                                pressed_combination.code = 125;
                                pressed_combination.meta = true;
                            }
                            code => {
                                pressed_combination.code = *code;
                            }
                        };
                        increment_val(&mut pressed_combinations, &pressed_combination);
                    }
                    2 => match code {
                        97 | 29 | 42 | 54 | 56 | 100 | 125 | 126 => continue,
                        code => {
                            pressed_combination.code = *code;
                            increment_val(&mut pressed_combinations, &pressed_combination);
                        }
                    },
                    _ => {}
                },
                _ => {
                    continue;
                }
            }

            if counter == 99 {
                debug!("100 keys were pressed; saving data to file...");
                counter = 0;
                let pressed_combinations_copy = pressed_combinations.clone();
                std::thread::spawn(|| match save_keys(pressed_combinations_copy) {
                    Ok(_) => {}
                    Err(e) => error!("Error in save_keys: {}", e),
                });
            } else {
                counter += 1;
            }
        }
    }
}

fn increment_val(
    map: &mut HashMap<PressedCombination, u32>,
    pressed_combination: &PressedCombination,
) {
    let count = map.entry(pressed_combination.clone()).or_insert(0);
    *count += 1;
}

fn save_keys(
    pressed_combinations: HashMap<PressedCombination, u32>,
) -> std::result::Result<(), String> {
    {
        let mut tmp_file =
            std::fs::File::create(TMP_FILE).map_err(|e| format!("Error creating tmp file: {}", e))?;
        let mut sorted_pressed_comb: Vec<(&PressedCombination, &u32)> =
            pressed_combinations.iter().collect();
        sorted_pressed_comb.sort_by(|a, b| b.1.cmp(a.1));
        for (p_comb, val) in sorted_pressed_comb {
            let key_name = code_to_keyname(p_comb.code);
            tmp_file
                .write_all(
                    format!(
                        "{} {} {} {} {}, {}\n",
                        val, key_name, p_comb.ctrl, p_comb.shift, p_comb.alt, p_comb.meta
                    ).as_bytes(),
                )
                .map_err(|e| format!("Error writing pressed_combinations to tmp file: {}", e))?;
        }
    }

    std::fs::rename(TMP_FILE, MAIN_FILE)
        .map_err(|e| format!("Error renaming tmp file to file: {}", e))?;
    Ok(())
}

fn move_previous_key_file() {
    if Path::new(MAIN_FILE).exists() {
        let unique_name = format!("{}", time::now().to_timespec().sec);
        let new_filename = Path::new(OLD_FILES_DIR).join(unique_name);
        match std::fs::rename(MAIN_FILE, &new_filename) {
            Ok(_) => {}
            Err(e) => println!("Error moving old file :(\n{}", e),
        }

        if new_filename.exists() {
            println!("Fucking really");
        }
    }
}

fn code_to_keyname(key: libc::__u16) -> &'static str {
    match key {
        1 => "Esc",
        2 => "1",
        3 => "2",
        4 => "3",
        5 => "4",
        6 => "5",
        7 => "6",
        8 => "7",
        9 => "8",
        10 => "9",
        11 => "0",
        12 => "-",
        13 => "=",
        14 => "Backspace",
        15 => "Tab",
        16 => "q",
        17 => "w",
        18 => "e",
        19 => "r",
        20 => "t",
        21 => "y",
        22 => "u",
        23 => "i",
        24 => "o",
        25 => "p",
        26 => "{",
        27 => "}",
        28 => "Enter",
        29 => "Ctrl",
        30 => "a",
        31 => "s",
        32 => "d",
        33 => "f",
        34 => "g",
        35 => "h",
        36 => "i",
        37 => "k",
        38 => "l",
        39 => ";",
        40 => "'",
        41 => "`",
        42 => "Shift",
        43 => "\\",
        44 => "z",
        45 => "x",
        46 => "c",
        47 => "v",
        48 => "b",
        49 => "n",
        50 => "m",
        51 => ",",
        52 => ".",
        53 => "/",
        54 => "Shift",
        55 => "*",
        56 => "Alt",
        57 => "Space",
        58 => "CapsLock",
        59 => "f1",
        60 => "f2",
        61 => "f3",
        62 => "f4",
        63 => "f5",
        64 => "f6",
        65 => "f7",
        66 => "f8",
        67 => "f9",
        68 => "f10",
        69 => "NumLock",
        70 => "ScrollLock",
        71 => "kp_7",
        72 => "kp_8",
        73 => "kp_9",
        74 => "kp_minus",
        75 => "kp_4",
        76 => "kp_5",
        77 => "kp_6",
        78 => "kp_plus",
        79 => "kp_1",
        80 => "kp_2",
        81 => "kp_3",
        82 => "kp_0",
        83 => "kp_dot",
        84 => "",
        85 => "",
        86 => "",
        87 => "f11",
        88 => "f12",
        89 => "",
        90 => "",
        91 => "",
        92 => "",
        93 => "",
        94 => "",
        95 => "",
        96 => "",
        97 => "",
        98 => "",
        99 => "",
        100 => "",
        101 => "",
        102 => "Home",
        103 => "Up",
        104 => "PageUp",
        105 => "Left",
        106 => "Right",
        107 => "End",
        108 => "Down",
        109 => "PageDown",
        110 => "Insert",
        111 => "Delete",
        112 => "",
        113 => "",
        114 => "",
        115 => "",
        116 => "",
        117 => "",
        118 => "",
        119 => "",
        120 => "",
        121 => "",
        122 => "",
        123 => "",
        124 => "",
        125 => "Meta",
        126 => "Meta",
        127 => "",
        128 => "",
        129 => "",
        130 => "",
        131 => "",
        132 => "",
        133 => "",
        134 => "",
        135 => "",
        136 => "",
        137 => "",
        138 => "",
        139 => "",
        140 => "",
        141 => "",
        142 => "",
        143 => "",
        144 => "",
        145 => "",
        146 => "",
        147 => "",
        148 => "",
        149 => "",
        150 => "",
        151 => "",
        152 => "",
        153 => "",
        154 => "",
        155 => "",
        156 => "",
        157 => "",
        158 => "",
        159 => "",
        160 => "",
        161 => "",
        162 => "",
        163 => "",
        164 => "",
        165 => "",
        166 => "",
        167 => "",
        168 => "",
        169 => "",
        170 => "",
        171 => "",
        172 => "",
        173 => "",
        174 => "",
        175 => "",
        176 => "",
        177 => "",
        178 => "",
        179 => "",
        180 => "",
        181 => "",
        182 => "",
        183 => "",
        184 => "",
        185 => "",
        186 => "",
        187 => "",
        188 => "",
        189 => "",
        190 => "",
        191 => "",
        192 => "",
        193 => "",
        194 => "",
        195 => "",
        196 => "",
        197 => "",
        198 => "",
        199 => "",
        _ => "ERROR_KEY",
    }
}
