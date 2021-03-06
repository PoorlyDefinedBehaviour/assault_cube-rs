use std::ffi::c_void;

use windows::{
  core::PCSTR,
  Win32::{
    Foundation::{GetLastError, BOOL, HINSTANCE, HWND, RECT},
    Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect, GetDC, HBRUSH, HDC},
    System::{
      Console::{AllocConsole, FreeConsole},
      LibraryLoader::{DisableThreadLibraryCalls, FreeLibraryAndExitThread, GetModuleHandleA},
    },
    UI::{
      Input::KeyboardAndMouse::GetAsyncKeyState,
      WindowsAndMessaging::{FindWindowA, GetWindowInfo, WINDOWINFO},
    },
  },
};

use anyhow::{Ok, Result};

const LOCAL_PLAYER_OFFSET: usize = 0x109B74;
const POSITION_X_FROM_LOCAL_PLAYER: usize = 0x4;
const POSITION_Y_FROM_LOCAL_PLAYER: usize = 0x8;
const POSITION_Z_FROM_LOCAL_PLAYER: usize = 0xC;
const HEALTH_OFFSET_FROM_LOCAL_PLAYER: usize = 0xf8;
const NAME_OFFSET_FROM_LOCAL_PLAYER: usize = 0x225;
const NUMBER_OF_PLAYERS_IN_MATCH_OFFSET: usize = 0x10F500;

const TEAM_OFFSET_FROM_LOCAL_PLAYER: usize = 0x32c;

const VIEW_MATRIX_ADDR: usize = 0x0501ae8;
const ENTITY_LIST_OFFSET: usize = 0x10f4f8;

const RED: u32 = 0x0000FF;
const BLUE: u32 = 0xFF0000;

#[derive(Debug, Clone, PartialEq)]
struct Vec3 {
  pub x: f32,
  pub y: f32,
  pub z: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Vec4 {
  pub x: f32,
  pub y: f32,
  pub z: f32,
  pub w: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Vec2 {
  pub x: f32,
  pub y: f32,
}

fn world_to_screen(
  position: Vec3,
  screen: &mut Vec2,
  view_matrix: [f32; 16],
  window_width: i32,
  window_height: i32,
) -> bool {
  let clip_coords = Vec4 {
    x: position.x * view_matrix[0]
      + position.y * view_matrix[4]
      + position.z * view_matrix[8]
      + view_matrix[12],
    y: position.x * view_matrix[1]
      + position.y * view_matrix[5]
      + position.z * view_matrix[9]
      + view_matrix[13],
    z: position.x * view_matrix[2]
      + position.y * view_matrix[6]
      + position.z * view_matrix[10]
      + view_matrix[14],
    w: position.x * view_matrix[3]
      + position.y * view_matrix[7]
      + position.z * view_matrix[11]
      + view_matrix[15],
  };

  if clip_coords.w < 0.1 {
    return false;
  }

  let normalized_device_coordinates = Vec3 {
    x: clip_coords.x / clip_coords.w,
    y: clip_coords.y / clip_coords.w,
    z: clip_coords.z / clip_coords.w,
  };

  screen.x = ((window_width / 2) as f32 * normalized_device_coordinates.x)
    + (normalized_device_coordinates.x + (window_width / 2) as f32);
  screen.y = -((window_height / 2) as f32 * normalized_device_coordinates.y)
    + (normalized_device_coordinates.y + (window_height / 2) as f32);

  true
}

struct Entity {
  pub entity_starts_at_addr: usize,
}

impl Entity {
  pub fn from_addr(addr: usize) -> Entity {
    Entity {
      entity_starts_at_addr: addr,
    }
  }

  pub fn health(&self) -> i32 {
    unsafe { *((self.entity_starts_at_addr + HEALTH_OFFSET_FROM_LOCAL_PLAYER) as *const i32) }
  }

  pub fn is_alive(&self) -> bool {
    self.health() > 0
  }

  pub fn position(&self) -> Vec3 {
    unsafe {
      Vec3 {
        x: *((self.entity_starts_at_addr + POSITION_X_FROM_LOCAL_PLAYER) as *const f32),
        y: *((self.entity_starts_at_addr + POSITION_Y_FROM_LOCAL_PLAYER) as *const f32),
        z: *((self.entity_starts_at_addr + POSITION_Z_FROM_LOCAL_PLAYER) as *const f32),
      }
    }
  }

  pub fn team(&self) -> i32 {
    unsafe { *((self.entity_starts_at_addr + TEAM_OFFSET_FROM_LOCAL_PLAYER) as *const i32) }
  }

  pub fn name(&self) -> String {
    let buffer = unsafe {
      *((self.entity_starts_at_addr + NAME_OFFSET_FROM_LOCAL_PLAYER) as *const [u8; 256])
    };

    let name_ends_at_index = {
      let mut i = 0;

      while buffer[i] != 0 {
        i += 1;
      }

      i
    };

    let name = &buffer[0..name_ends_at_index];

    String::from_utf8_lossy(name).to_string()
  }
}

fn entrypoint() -> Result<()> {
  let module_base_addr = unsafe {
    GetModuleHandleA("ac_client.exe")
      .map(|hinstance| hinstance.0 as usize)
      .expect("error getting module handle")
  };

  println!(
    "got module base address. base_address={:#x}",
    module_base_addr
  );

  unsafe {
    let view_matrix = VIEW_MATRIX_ADDR as *const [f32; 16];

    let window_handle = FindWindowA(PCSTR(std::ptr::null()), "AssaultCube");

    let hdc = GetDC(window_handle);

    let local_player = Entity::from_addr(*((module_base_addr + LOCAL_PLAYER_OFFSET) as *mut usize));

    let num_players_in_match =
      *((module_base_addr + NUMBER_OF_PLAYERS_IN_MATCH_OFFSET) as *const i32) as usize;

    let entity_list_ptr = *((module_base_addr + ENTITY_LIST_OFFSET) as *const u32);

    let red_brush = CreateSolidBrush(RED);
    let blue_brush = CreateSolidBrush(BLUE);

    let window = get_window_dimensions(window_handle)?;

    loop {
      const VK_DELETE: i32 = 0x2E;
      if GetAsyncKeyState(VK_DELETE) & 1 == 1 {
        // Leave the loop and deject dll
        println!("quiting");
        break;
      }

      // Skip the first entity list position because it is always empty.
      for i in 1..num_players_in_match {
        let entity = Entity::from_addr(*((entity_list_ptr as usize + i * 0x4) as *const usize));

        if !entity.is_alive() {
          continue;
        }

        let mut screen = Vec2 { x: 0.0, y: 0.0 };

        // If entity cannot be mapped to 2d?
        if !world_to_screen(
          entity.position(),
          &mut screen,
          *view_matrix,
          window.width,
          window.height,
        ) {
          continue;
        }

        let entity_position = entity.position();

        let distance = calculate_3d_distance(local_player.position(), entity_position);

        let width = window.width as f32 / distance;

        let height = window.height as f32 / distance;

        let box_brush_color = if local_player.team() == entity.team() {
          blue_brush
        } else {
          red_brush
        };

        draw_border_box(
          hdc,
          box_brush_color,
          (screen.x - width / 2.0) as i32,
          (screen.y - height) as i32,
          width as i32,
          height as i32,
          1,
        );
      }
    }

    DeleteObject(red_brush);
    DeleteObject(blue_brush);
  }

  Ok(())
}

fn calculate_3d_distance(pos_a: Vec3, pos_b: Vec3) -> f32 {
  (((pos_a.x - pos_b.x) * (pos_a.x - pos_b.x))
    + ((pos_a.y - pos_b.y) * (pos_a.y - pos_b.y))
    + ((pos_a.z - pos_b.z) * (pos_a.z - pos_b.z)))
    .sqrt()
}

#[derive(Debug)]
struct WindowDimensions {
  pub width: i32,
  pub height: i32,
}

fn get_window_dimensions(window: HWND) -> Result<WindowDimensions> {
  let mut window_info = WINDOWINFO::default();
  window_info.cbSize = std::mem::size_of_val(&window_info) as u32;

  unsafe {
    if GetWindowInfo(window, &mut window_info).as_bool() {
      Ok(WindowDimensions {
        width: window_info.rcClient.right - window_info.rcClient.left,
        height: window_info.rcClient.bottom - window_info.rcClient.top,
      })
    } else {
      Err(anyhow::anyhow!(
        "error getting screen dimensions. last_error={:?}",
        GetLastError()
      ))
    }
  }
}

fn draw_filled_rect(hdc: HDC, brush: HBRUSH, x: i32, y: i32, width: i32, height: i32) {
  let rect = RECT {
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
  };
  unsafe {
    FillRect(hdc, &rect as _, brush);
  }
}

fn draw_border_box(
  hdc: HDC,
  brush: HBRUSH,
  x: i32,
  y: i32,
  width: i32,
  height: i32,
  thickness: i32,
) {
  draw_filled_rect(hdc, brush, x, y, width, thickness);

  draw_filled_rect(hdc, brush, x, y, thickness, height);

  draw_filled_rect(hdc, brush, x + width, y, thickness, height);

  draw_filled_rect(hdc, brush, x, y + height, width + thickness, thickness);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
  dll_module: HINSTANCE,
  call_reason: u32,
  _reserved: *mut c_void,
) -> BOOL {
  const DLL_PROCESS_ATTACH: u32 = 1;

  if call_reason == DLL_PROCESS_ATTACH {
    unsafe {
      DisableThreadLibraryCalls(dll_module).expect("error disabling thread library calls");

      std::thread::spawn(move || {
        AllocConsole();
        println!("entrypoint returned: {:?}", entrypoint());
        use std::process::Command;
        let _ = Command::new("cmd.exe").arg("/c").arg("pause").status();
        FreeConsole();
        FreeLibraryAndExitThread(dll_module, 0);
      });
    }
  }

  BOOL::from(true)
}
