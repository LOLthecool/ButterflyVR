extends VBoxContainer
class_name SignInMenu

const SIGNIN_ENDPOINT: String = "/api/v0/token"

## WARNING: changing the constants in this region could stop all users from signing in
#region DANGER
# since the salt for the client side hash must be something the client knows,
# we use the email as the unique part of the hash, but since emails are often reused,
# we append this application specific value to the salt to ensure different platforms,
# have different salts for the same user.
const PASSWORD_SALT_CONST_HALF: String = "6uplNKoY38xV81Cl"
# argon2 parameters
const MEMORY: int = 512
const ITERATIONS: int = 1
const PARALLELISM: int = 4
const OUTPUT_LENGTH: int = 64
#endregion

@export var greeter:GreeterMenu
@export var loading_screen:LoadingMenu

@export var email:LineEdit
@export var password:LineEdit
@export var remember:CheckBox
@export var cancel_button:Button
@export var continue_button:Button

@export var invalid_email_error:Label
@export var password_length_error:Label
@export var incorrect_error:Label
@export var rate_limit_error:Label
@export var banned_error:Label

@export var generic_error_header:Label
@export var generic_error_code:Label
@export var generic_error_message:Label

func hide_all_errors() -> void:
	invalid_email_error.visible = false
	password_length_error.visible = false
	incorrect_error.visible = false
	rate_limit_error.visible = false
	banned_error.visible = false
	banned_error.text = "UNNAMED"
	generic_error_header.visible = false
	generic_error_code.visible = false
	generic_error_code.text = "UNNAMED"
	generic_error_message.visible = false
	generic_error_message.text = "UNNAMED"

func populate_from_signup(email:String, password:String) -> void:
	self.email.text = email
	self.password.text = password


func _on_cancel_pressed() -> void:
	greeter.visible = true


func _on_sign_up_pressed() -> void:
	hide_all_errors()
	
	var failed:bool = false
	
	# matches "1 or more characters, '@', 1 or more characters, '.', 1 or more characters"
	# email is properly verified server side
	if !email.text.match("?*@?*.?*") or email.text.length() > 128:
		invalid_email_error.visible = true
		failed = true
	if password.text.length() < 6:
		password_length_error.visible = true
		failed = true
	
	if failed:
		return

	cancel_button.disabled = true
	continue_button.disabled = true


	## WARNING: changing this code could prevent users from logging in
	var client_salt: String = PASSWORD_SALT_CONST_HALF + email.text

	var thread: Thread = Thread.new()
	thread.start(
		Argon2Hasher
		.hash
		.bind(MEMORY, ITERATIONS, PARALLELISM, password.text, client_salt, OUTPUT_LENGTH)
	)
	
	while !thread.is_started():
		await get_tree().physics_frame

	while thread.is_alive():
		await get_tree().physics_frame
	var password_hash: PackedByteArray = thread.wait_to_finish()

	var body: String = JSON.stringify(
		{ "email": email.text, "password_hash": password_hash as Array[int], "allow_renew": remember.button_pressed }
	)

	var response:Array = await GlobalAPIHandler \
			.make_request(HTTPClient.METHOD_POST, SIGNIN_ENDPOINT, PackedStringArray(), body)

	@warning_ignore("unsafe_call_argument")
	var result: Array = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[HTTPClient.RESPONSE_OK],
		["token", "token_expires", "renewable"],
	)
	if result[0]:
		var data: Dictionary = result[4]
		var token: Array[int] = []
		@warning_ignore("unsafe_cast")
		token.assign(data["token"] as Array)
		@warning_ignore("unsafe_cast")
		await GlobalAccountHandler.set_token(
			token,
			data["token_expires"] as int,
			data["renewable"] as bool,
		)
		await loading_screen.start()
	else:
		var response_code:int = result[1]
		var error_code: String = result[2]
		var error_message: String = result[3]
		
		if response_code == 429:
			rate_limit_error.visible = true
		
		if error_code == "DosentExist":
			incorrect_error.visible = true
		
		if error_code == "Banned":
			banned_error.text = error_message
			banned_error.visible = true
		
		generic_error_header.visible = true
		if error_code != "":
			generic_error_code.text = "Error Code: " + error_code
			generic_error_code.visible = true
		if error_message != "":
			generic_error_message.text = "Error Message: " + error_message
			generic_error_message.visible = true
		return
