extends Control
# handles login and registering
# once it signs in with a valid token transitions to the home world
# for a new user their homeworld is the tutorial world

const GREETER_TAB:int = 1
const LOADING_TAB:int = 0
const SIGNIN_TAB:int = 2
const REGISTER_TAB:int = 3

const REGISTER_ENDPOINT:String = "/api/v0/user"
const SIGNIN_ENDPOINT:String = "/api/v0/token"

const TOSLOCATION:String = "res://scenes/startup/TermsOfService.txt"
const PRIVACYPOLICYLOCATION:String = "res://scenes/startup/PrivacyPolicy.txt"

## WARNING: changing the constants in this region could stop all users from signing in
#region DANGER
# since the salt for the client side hash must be something the client knows,
# we use the email as the unique part of the hash, but since emails are often reused,
# we append this application specific value to the salt to ensure different platforms,
# have different salts for the same user.
# this salt is not ideal so we should never store the client side hashed password
const PASSWORD_SALT_CONST_HALF:String = "6uplNKoY38xV81Cl"
# argon2 parameters
const MEMORY:int = 512
const ITERATIONS:int = 1
const PARALLELISM:int = 4
const OUTPUT_LENGTH:int = 64
#endregion

@export var last_screen:int = GREETER_TAB
@export var tab_container:TabContainer

@export var popup:Panel
@export var popup_text:RichTextLabel
@export var popup_button:Button

@export var loading_text:Label

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

func _init() -> void:
	if OS.get_cmdline_args().has("--server") or OS.get_cmdline_args().has("--headless"):
		ServerLoader.start()
		queue_free()

func _ready() -> void:
	if OS.get_cmdline_args().has("--server") or OS.get_cmdline_args().has("--headless"):
		return
	
	if await GlobalAccountHandler.check_token_valid():
		loading_text.text = "Logging in with saved account..."
		start()
	else:
		tab_container.current_tab = GREETER_TAB

# handles initial loading of the homeworld
# must have a valid token by this point
func start() -> void:
	tab_container.current_tab = LOADING_TAB
	loading_text.text = "Loading homeworld..."
	await get_tree().create_timer(1).timeout # give user a chance to cancel load
	if load_cancelled:
		tab_container.current_tab = last_screen
		return
	GlobalWorldHandler.load_homeworld()

func _on_register_selected() -> void:
	tab_container.current_tab = REGISTER_TAB
	last_screen = GREETER_TAB

func _on_login_selected() -> void:
	tab_container.current_tab = SIGNIN_TAB
	last_screen = GREETER_TAB

func show_popup(text:String, go_last_screen:bool = false) -> void:
	push_warning(text)
	popup_text.text = text
	popup.visible = true
	await popup_button.pressed
	if go_last_screen:
		tab_container.current_tab = last_screen
	popup.visible = false

func _on_register() -> void:
	if register_username.text.length() < 3:
		await show_popup("Username must be at least 3 characters")
		return
	if register_username.text.length() > 32:
		await show_popup("Username must be 32 characters or shorter")
		return
	# matches "1 or more characters, '@', 1 or more characters, '.', 1 or more characters"
	# email is properly verified server side
	if !register_email.text.match("?*@?*.?*"):
		await show_popup("Invalid email")
		return
	if !register_email.text.length() > 128:
		await show_popup("email is too long")
		return
	if register_password.text.length() < 6:
		await show_popup("Password must be longer than 6 characters")
		return
	if !register_tos.button_pressed:
		await show_popup("Please agree to the terms of service and privacy policy")
		return
	if !register_acknowledge.button_pressed:
		await show_popup("Please accept the alpha disclaimer")
		return
	
	last_screen = tab_container.current_tab
	tab_container.current_tab = LOADING_TAB
	
	loading_text.text = "Hashing password..."
	await get_tree().physics_frame
	await get_tree().physics_frame
	
	var username:String = register_username.text
	var email:String = register_email.text
	var password:String = register_password.text
	
	## WARNING: changing this code could prevent users from logging in or creating accounts
	var client_salt:String = PASSWORD_SALT_CONST_HALF + email
	var password_hash:PackedByteArray = Argon2Hasher.hash(MEMORY, ITERATIONS, PARALLELISM, password, client_salt, OUTPUT_LENGTH)
	
	loading_text.text = "Contacting server..."
	
	var body:String = JSON.stringify({"username": username, "email": email, "password_hash": password_hash as Array[int]})
	GlobalAPIHandler.make_request(HTTPClient.METHOD_POST, REGISTER_ENDPOINT, PackedStringArray(), body).connect(on_register_response)

func on_register_response(code:HTTPClient.ResponseCode, _headers:PackedStringArray, body:String) -> void:
	var result:Array = GlobalAPIHandler.handle_response(code, body, [HTTPClient.RESPONSE_OK], [])
	if result[0]:
		last_screen = SIGNIN_TAB
		await show_popup("Account created. Click the verify link in your emails before signing in.", true)
		last_screen = GREETER_TAB
	else:
		var response_code:int = result[1]
		var error_code:String = result[2]
		var error_message:String = result[3]
		var message:String = "Failed to create account."
		if response_code != -1:
			message += "\nserver response: %s" % response_code
		if error_code != "":
			message += "\nError code: %s" % (error_code)
		if error_message != "":
			message += "\nError message: \n%s" % (error_message)
		await show_popup(message, true)

func _on_login() -> void:
	# matches "1 or more characters, '@', 1 or more characters, '.', 1 or more characters"
	if !signin_email.text.match("?*@?*.?*"):
		await show_popup("Invalid email")
		return
	if signin_password.text.length() < 6:
		await show_popup("Invalid password")
		return
	
	last_screen = tab_container.current_tab
	tab_container.current_tab = LOADING_TAB
	
	loading_text.text = "Hashing password..."
	
	var email:String = signin_email.text
	var password:String = signin_password.text
	var remember:bool = signin_remember.button_pressed
	
	## WARNING: changing this code could prevent users from logging in
	var client_salt:String = PASSWORD_SALT_CONST_HALF + email
	
	var thread:Thread = Thread.new()
	thread.start(Argon2Hasher.hash.bind(MEMORY, ITERATIONS, PARALLELISM, password, client_salt, OUTPUT_LENGTH))
	
	while thread.is_alive():
		await get_tree().physics_frame
	var password_hash:PackedByteArray = thread.wait_to_finish()
	
	loading_text.text = "Contacting server..."
	
	var body:String = JSON.stringify({"email": email, "password_hash": password_hash as Array[int], "allow_renew": remember})

	GlobalAPIHandler.make_request(HTTPClient.METHOD_POST, SIGNIN_ENDPOINT, PackedStringArray(), body).connect(on_login_response)

func on_login_response(code:HTTPClient.ResponseCode, _headers:PackedStringArray, body:String) -> void:
	var result:Array = GlobalAPIHandler.handle_response(code, body, [HTTPClient.RESPONSE_OK], [
			"token",
			"token_expires",
			"renewable"
			])
	if result[0]:
		var data:Dictionary = result[4]
		var token:Array[int] = []
		@warning_ignore("unsafe_cast")
		token.assign(data["token"] as Array)
		@warning_ignore("unsafe_cast")
		GlobalAccountHandler.set_token(
				token,
				data["token_expires"] as int,
				data["renewable"] as bool
				)
		start()
	else:
		var response_code:int = result[1]
		var error_code:String = result[2]
		var error_message:String = result[3]
		var message:String = "Failed to log in."
		if response_code == -1:
			message += "\nServer did not send a response."
		else:
			message += "\nResponse code: %s" % (response_code)
		if error_code != "":
			message += "\nError code: %s" % (error_code)
		if error_message != "":
			message += "\nError message: \n%s" % (error_message)
		await show_popup(message, true)
		return

func _on_back_button_pressed() -> void:
	tab_container.current_tab = last_screen
	last_screen = GREETER_TAB


# handle showing the terms of service and privacy policy
# todo: should probably make a generic popup handler and use that later
func _on_link_clicked(meta: Variant) -> void:
	if meta == "local.privacy":
		show_popup(FileAccess.get_file_as_string(PRIVACYPOLICYLOCATION))
	elif meta == "local.TOS":
		show_popup(FileAccess.get_file_as_string(TOSLOCATION))
	else:
		push_warning("got link to unknown content: ", meta)


func _on_load_cancelled() -> void:
	load_cancelled = true
