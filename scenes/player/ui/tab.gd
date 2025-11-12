extends PanelContainer

signal tab_destroyed
signal tab_clicked(tab:Control)

@export var tab_button:Button

var held_tab:Control

func create(tab_name:String, tab:Control) -> void:
	tab_button.text = tab_name
	held_tab = tab


func _on_tab_clicked() -> void:
	tab_clicked.emit(held_tab)


func _on_close() -> void:
	held_tab.queue_free()
	tab_destroyed.emit()
