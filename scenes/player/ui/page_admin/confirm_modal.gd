extends Node
class_name ConfirmModal

signal complete(continued:bool)

@export var message:Label
@export var root:Control

func show(msg:String) -> bool:
	message.text = msg
	root.visible = true
	return await complete


func on_cancel() -> void:
	root.visible = false
	complete.emit(false)


func on_confirm() -> void:
	root.visible = false
	complete.emit(true)
