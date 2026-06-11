extends ScrollContainer

@export var message_container:VBoxContainer

var chat_message:PackedScene = preload("res://scenes/player/ui/page_chat/message.tscn")
var username_cache:Dictionary[PackedByteArray, String] = {}

func _ready() -> void:
	var message_history:Array[ChatBoxManager.Message] = GlobalWorldHandler.current_world.chat_box_manager.messages.duplicate()
	for message:ChatBoxManager.Message in message_history:
		@warning_ignore("unsafe_cast")
		on_message.call_deferred(message as ChatBoxManager.Message)
	GlobalWorldHandler.current_world.chat_box_manager.new_message_sent.connect.call_deferred(on_message)

func on_message(message:ChatBoxManager.Message) -> void:
	var instance:ChatMessage = chat_message.instantiate()
	instance.visible = false
	message_container.add_child(instance)
	if username_cache.get(message.player) == null:
		username_cache[message.player] = ""
		username_cache[message.player] = await APIHelper.get_username(UUID.from_bytes(message.player))
	while username_cache[message.player] == "":
		await get_tree().physics_frame
	instance.configure(username_cache[message.player], message.text, PlaceholderTexture2D.new())
	instance.visible = true
