extends Control
# handles login and registering
# once it signs in with a valid token transitions to the home world
# for a new user their homeworld is the tutorial world

const GREETER_TAB:int = 1
const LOADING_TAB:int = 0
const SIGNIN_TAB:int = 2
const REGISTER_TAB:int = 3

@export var last_screen:int
@export var tab_container:TabContainer

func _ready() -> void:
	if GlobalAccountHandler.session_token == []:
		tab_container.current_tab = GREETER_TAB
	else:
		start()

# handles initial loading of the homeworld
func start() -> void:
	pass

func _on_register_selected() -> void:
	pass # Replace with function body.

func _on_login_selected() -> void:
	pass # Replace with function body.


func _on_register() -> void:
	last_screen = tab_container.current_tab
	pass # Replace with function body.

func _on_login() -> void:
	last_screen = tab_container.current_tab
	pass # Replace with function body.


func _on_back_button_pressed() -> void:
	tab_container.current_tab = last_screen
	last_screen = GREETER_TAB


# handle showing the terms of service and privacy policy
# links starting with .local with no tld are treated as internally available resources
# otherwise we pass to the browser
# todo: should probably make a generic popup handler and use that later
func _on_link_clicked(meta: Variant) -> void:
	pass # Replace with function body.
