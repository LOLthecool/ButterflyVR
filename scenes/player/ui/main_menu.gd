extends VBoxContainer

@export var ui_access:UiAccess

var previous_mouse_mode:Input.MouseMode

func _ready() -> void:
	visible = false

func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("player_mainmenu_toggle"):
		if visible:
			visible = false
			ui_access.menu_opened = false
			Input.mouse_mode = Input.MOUSE_MODE_VISIBLE if previous_mouse_mode == null else previous_mouse_mode
		else:
			visible = true
			ui_access.menu_opened = true
			previous_mouse_mode = Input.mouse_mode
			Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
