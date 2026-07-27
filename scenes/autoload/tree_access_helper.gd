extends Node
class_name TreeAccessHelper

signal physics_frame
signal process_frame

func _physics_process(_delta: float) -> void:
	physics_frame.emit()

func _process(_delta: float) -> void:
	process_frame.emit()
