extends Control
# handles login and registering
# once it signs in with a valid token transitions to the home world
# for a new user their homeworld is the tutorial world

const GREETER_TAB:int = 1
const LOADING_TAB:int = 0
const SIGNIN_TAB:int = 2
const REGISTER_TAB:int = 3
const REGISTER_ENDPOINT:String = "API/V0/user"
const SIGNIN_ENDPOINT:String = "API/V0/token"

## warning: changing the constant in this region could stop all users from signing in
#region DANGER
# since the salt for the client side hash must be something the client knows,
# we use the email as the unique part of the hash, but since emails are often reused,
# we append this application specific value to the salt to ensure different platforms,
# have different salts for the same user.
# this salt is not ideal so we should never store the client side hashed password
const PASSWORD_SALT_CONST_HALF:String = "ButterflyVR"
# argon2 parameters
const MEMORY:int = 48
const ITERATIONS:int = 16
const PARALLELISM:int = 1
const OUTPUT_LENGTH:int = 64
#endregion

@export var last_screen:int = GREETER_TAB
@export var tab_container:TabContainer

#region register_vars

@export var register_username:LineEdit
@export var register_email:LineEdit
@export var register_password:LineEdit
@export var register_tos:CheckBox
@export var register_acknowledge:CheckBox
#endregion

#region signin_vars

@export var signin_email:LineEdit
@export var signin_password:LineEdit
@export var signin_remember:CheckBox
#endregion

var load_cancelled:bool = false

func _ready() -> void:
	if GlobalAccountHandler.token_valid:
		start()
	else:
		tab_container.current_tab = GREETER_TAB

# handles initial loading of the homeworld
func start() -> void:
	await get_tree().create_timer(1.2).timeout # give user a chance to cancel load

func _on_register_selected() -> void:
	tab_container.current_tab = REGISTER_TAB

func _on_login_selected() -> void:
	tab_container.current_tab = SIGNIN_TAB


func _on_register() -> void:
	# todo: some basic validity checks before sending to the server
	last_screen = tab_container.current_tab
	tab_container.current_tab = LOADING_TAB
	var username:String = register_username.text
	var email:String = register_email.text
	var password:String = register_password.text
	## warning: changing this code could prevent users from logging in or creating accounts
	var client_salt:String = PASSWORD_SALT_CONST_HALF + email
	var password_hash:PackedByteArray = Argon2Hasher.hash(MEMORY, ITERATIONS, PARALLELISM, password, client_salt, OUTPUT_LENGTH)
	var body:String = JSON.stringify({"username": username, "email": email, "password_hash": password_hash})
	GlobalAPIHandler.make_request(HTTPClient.METHOD_POST, REGISTER_ENDPOINT, PackedStringArray(), body).connect(on_register_response)

func on_register_response(code:HTTPClient.ResponseCode, headers:PackedStringArray, body:String) -> void:
	pass

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


func _on_load_cancelled() -> void:
	load_cancelled = true
