extends VBoxContainer
class_name PageHandler

const DEFAULT_PAGE:PackedScene = preload("res://scenes/player/ui/page_home/home_page.tscn")
const TAB:PackedScene = preload("res://scenes/player/ui/tab.tscn")

@export var page_root:ScrollContainer
@export var tab_container:HBoxContainer

var current_page:Page
var current_tab:Tab

func create_tab() -> void:
	var tab:Tab = TAB.instantiate()
	var page:Page = DEFAULT_PAGE.instantiate()
	tab.create(page)
	tab.tab_clicked.connect(on_tab_clicked)
	tab.tab_destroyed.connect(on_tab_destroyed)
	tab_container.add_child(tab)
	if current_page == null:
		current_tab = tab
		tab.hide_overlay()
		current_page = page
		page_root.add_child(current_page)
	else:
		on_tab_clicked(tab, page)

func change_current_page(page:Page) -> void:
	if current_tab == null:
		create_tab()
	current_tab.held_tab.queue_free()
	current_tab.held_tab = page
	page.tab_name_changed.connect(current_tab.update_name)
	on_tab_clicked(current_tab, page)

func on_tab_clicked(tab:Tab, page:Page) -> void:
	if page == current_page:
		return
	page_root.remove_child(current_page)
	page_root.add_child(page)
	current_page = page
	current_tab.show_overlay()
	tab.hide_overlay()
	current_tab = tab

func on_tab_destroyed(tab:Tab, page:Page) -> void:
	if page != current_page:
		return
	var idx:int = tab.get_index()
	if tab_container.get_children().size() > idx + 1:
		var new_tab:Tab = tab_container.get_child(idx + 1)
		on_tab_clicked(new_tab, new_tab.held_tab)
	elif idx - 1 >= 0:
		var new_tab:Tab = tab_container.get_child(idx - 1)
		on_tab_clicked(new_tab, new_tab.held_tab)
	else:
		page_root.remove_child(current_page)
		current_page = null
		current_tab = null
