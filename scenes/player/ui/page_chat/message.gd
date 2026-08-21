extends HBoxContainer
class_name ChatMessage

@export var image: TextureRect
@export var player_label: Label
@export var message_label: Label


func configure(player: String, message: String, icon: Texture2D) -> void:
	image.texture = icon
	player_label.text = player + ":"
	message_label.text = message
