extends Camera3D
class_name DesktopCamera

const VERTICAL_LIMIT_MAX: float = deg_to_rad(90)
const VERTICAL_LIMIT_MIN: float = deg_to_rad(-80)
const MAX_HEAD_Y_ROTATION_DEGREES: float = 70
const MAX_HEAD_Y_ROTATION: float = deg_to_rad(MAX_HEAD_Y_ROTATION_DEGREES)

@export var player_access: PlayerAccess
@export var raycaster: Node3D
@export var movement_handler: MovementHandler
@export var rotation_center: Node3D

var sensitivity: float = 0.0005
var verticalSensitivityMultiplier: float = 0.75
var head_target_ready: bool = false

@onready var horizontalSensitivity: float = sensitivity
@onready var verticalSensitivity: float = sensitivity * verticalSensitivityMultiplier


func _ready() -> void:
	make_current()
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	player_access.player.interactor_origin = raycaster
	player_access.player.avatar_changed.connect(on_avatar_changed)


func on_avatar_changed() -> void:
	if player_access.player.head_ik_target:
		rotation_center.global_position = player_access.player.head_ik_target.global_position
	else:
		rotation_center.position.y = 1
	position = player_access.player.head_view_offset
	if movement_handler.player_state == MovementHandler.Player_states.CROUCHED:
		movement_handler.player_state = MovementHandler.Player_states.NONE
	movement_handler.camera_height = rotation_center.position.y
	head_target_ready = true


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion and Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:
		move_cam(
			(event as InputEventMouseMotion).relative.x,
			(event as InputEventMouseMotion).relative.y,
		)


func move_cam(xrot: float, yrot: float) -> void:
	rotation_center.rotate_y(-xrot * horizontalSensitivity)
	rotation_center.rotation.x = clamp(
		rotation_center.rotation.x - (yrot * verticalSensitivity),
		VERTICAL_LIMIT_MIN,
		VERTICAL_LIMIT_MAX,
	)


func _physics_process(delta: float) -> void:
	if !current:
		make_current()

	if head_target_ready and player_access.player.head_ik_target:
		var movement: Vector2 = Vector2(
			player_access.player.velocity.x,
			player_access.player.velocity.z,
		)
		if movement != Vector2.ZERO:
			var diff: float = rotation_center.rotation.y
			player_access.player.rotation.y = move_toward(
				player_access.player.rotation.y,
				player_access.player.rotation.y + diff,
				delta * absf(diff) * 9,
			)
			rotation_center.rotation.y = move_toward(
				rotation_center.rotation.y,
				rotation_center.rotation.y - diff,
				delta * absf(diff) * 9,
			)
			return

		if rotation_center.rotation.y > MAX_HEAD_Y_ROTATION:
			var diff: float = rotation_center.rotation.y - MAX_HEAD_Y_ROTATION
			player_access.player.rotation.y += diff
			rotation_center.rotation.y -= diff
		if rotation_center.rotation.y < -MAX_HEAD_Y_ROTATION:
			var diff: float = rotation_center.rotation.y + MAX_HEAD_Y_ROTATION
			player_access.player.rotation.y += diff
			rotation_center.rotation.y -= diff
		player_access.player.head_ik_target.rotation = rotation_center.rotation
