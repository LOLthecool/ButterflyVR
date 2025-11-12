extends VBoxContainer

const DEFAULT_PAGE:PackedScene = preload("res://scenes/player/ui/page_home/home_page.tscn")

@export var page_root:ScrollContainer
@export var tab_container:HBoxContainer

var current_tab:Control

func change_tab(tab_idx:int) -> void:
	pass

func create_tab() -> void:
	pass

func change_tab_page(page:Control, args:Dictionary[String, Variant] = {}) -> void:
	pass
