extends Node
class_name WorldController

@export var spawn_point:Node3D
@export var chat_box_manager:ChatBoxManager
@export var player_grab_handler:PlayerGrabHandler
@export var avatar_change_handler:AvatarChangeHandler
@export var interactable_handler:InteractableHandler

func _init() -> void:
	GlobalWorldHandler.current_world = self

func _ready() -> void:
	if NetworkManager.is_server():
		chat_box_manager.new_message_sent.connect(log_chat_to_console)

func log_chat_to_console(message:ChatBoxManager.Message) -> void:
	print(await APIHelper.get_username(UUID.from_bytes(message.player)) + ": " + message.text)

static func setup(spawn_point:Node3D) -> WorldController:
	var world:WorldController = preload("res://scenes/world/world_controller.tscn").instantiate()
	world.spawn_point = spawn_point
	
	return world
