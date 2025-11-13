extends Control
class_name Page

signal tab_name_changed(tab_name:String)

@export var page_name:String

func change_tab_name(tab_name:String) -> void:
	tab_name_changed.emit(tab_name)

func _ready() -> void:
	tab_name_changed.emit(page_name if page_name != "" else (name as String))
