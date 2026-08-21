extends PanelContainer
class_name Tab

signal tab_destroyed(tab: Tab, page: Page)
signal tab_clicked(tab: Tab, page: Page)

@export var tab_button: Button
@export var overlay: Panel

var held_tab: Page


func create(tab: Page) -> void:
	held_tab = tab
	held_tab.tab_name_changed.connect(update_name)


func update_name(tab_name: String) -> void:
	tab_button.text = tab_name


func show_overlay() -> void:
	overlay.visible = true


func hide_overlay() -> void:
	overlay.visible = false


func _on_tab_clicked() -> void:
	tab_clicked.emit(self, held_tab)


func _on_close() -> void:
	tab_destroyed.emit(self, held_tab)
	held_tab.queue_free()
	queue_free()
