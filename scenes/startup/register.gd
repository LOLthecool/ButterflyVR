extends VBoxContainer
class_name RegisterMenu

const REGISTER_ENDPOINT: String = "/api/v0/user"

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
@export var sign_in:SignInMenu
@export var transition:RegisterTransitionMenu

@export var username:LineEdit
@export var email:LineEdit
@export var password:LineEdit
@export var password2:LineEdit

@export var checkbox:CheckBox
@export var checkbox2:CheckBox

@export var cancel_button:Button
@export var continue_button:Button

@export var username_length_error:Label
@export var username_email_taken_error:Label
@export var email_error:Label
@export var password_length_error:Label
@export var password_match_error:Label
@export var agree_error:Label
@export var ack_error:Label

@export var generic_error_header:Label
@export var generic_error_code:Label
@export var generic_error_message:Label

func _on_cancel_pressed() -> void:
	greeter.visible = true

func hide_all_errors() -> void:
	username_length_error.visible = false
	username_email_taken_error.visible = false
	email_error.visible = false
	password_length_error.visible = false
	agree_error.visible = false
	ack_error.visible = false
	generic_error_header.visible = false
	generic_error_code.visible = false
	generic_error_code.text = "UNNAMED"
	generic_error_message.visible = false
	generic_error_message.text = "UNNAMED"

func _on_sign_up_pressed() -> void:
	hide_all_errors()
	
	var failed:bool = false
	
	if username.text.length() < 3 or username.text.length() > 32:
		username_length_error.visible = true
		failed = true
	# matches "1 or more characters, '@', 1 or more characters, '.', 1 or more characters"
	# email is properly verified server side
	if !email.text.match("?*@?*.?*") or email.text.length() > 128:
		email_error.visible = true
		failed = true
	if password.text.length() < 6:
		password_length_error.visible = true
		failed = true
	if password.text != password2.text:
		password_match_error.visible = true
		failed = true
	if !checkbox.button_pressed:
		agree_error.visible = true
		failed = true
	if !checkbox2.button_pressed:
		ack_error.visible = true
		failed = true
	
	if failed:
		return
	
	cancel_button.disabled = true
	continue_button.disabled = true

	## WARNING: changing this code could prevent users from logging in or creating accounts
	var client_salt: String = PASSWORD_SALT_CONST_HALF + email.text
	
	var thread:Thread = Thread.new()
	thread.start(func() -> PackedByteArray:
		return Argon2Hasher.hash(
				MEMORY,
				ITERATIONS,
				PARALLELISM,
				password.text,
				client_salt,
				OUTPUT_LENGTH,
			))
	
	while !thread.is_started():
		await get_tree().physics_frame
	
	while thread.is_alive():
		await get_tree().physics_frame
	
	var password_hash: PackedByteArray = thread.wait_to_finish()

	var body: String = JSON.stringify(
		{ "username": username.text, "email": email.text, "password_hash": password_hash as Array[int] }
	)
	var response:Array = await GlobalAPIHandler \
			.make_request(HTTPClient.METHOD_POST, REGISTER_ENDPOINT, PackedStringArray(), body)
	@warning_ignore("unsafe_call_argument")
	var result: Array = GlobalAPIHandler.handle_response(response[0], response[2], [200], [])
	
	cancel_button.disabled = false
	continue_button.disabled = false
	
	if result[0]:
		sign_in.populate_from_signup(email.text, password.text)
		transition.visible = true
	else:
		var error_code: String = result[2]
		var error_message: String = result[3]
		
		if error_code == "AlreadyExists":
			username_email_taken_error.visible = true
		
		generic_error_header.visible = true
		if error_code != "":
			generic_error_code.text = "Error Code: " + error_code
			generic_error_code.visible = true
		if error_message != "":
			generic_error_message.text = "Error Message: " + error_message
			generic_error_message.visible = true
		return
	


func _on_rich_text_label_meta_clicked(meta: Variant) -> void:
	OS.shell_open(str(meta))
