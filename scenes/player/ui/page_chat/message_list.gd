extends ScrollContainer

@export var message_container:VBoxContainer

var chat_message:PackedScene = preload("res://scenes/player/ui/page_chat/message.tscn")

func _ready() -> void:
	GlobalWorldHandler.current_world.chat_box_manager.new_message_sent.connect(on_message)

func on_message(message:ChatBoxManager.Message) -> void:
	print("message got")
	var player_name:String = await APIHelper.get_username(UUID.from_bytes(message.player))
	var instance:ChatMessage = chat_message.instantiate()
	instance.configure(player_name, message.text, PlaceholderTexture2D.new())
	message_container.add_child(instance)
