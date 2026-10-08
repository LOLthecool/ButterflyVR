extends Node
class_name UserBanModal

const MODERATION_MODERATE_USER_ROUTE: String = "/api/v0/mod/moderate_user"
const MINUTE: int = 60
const HOUR: int = 60 * MINUTE
const DAY: int = 24 * HOUR
const WEEK: int = 7 * DAY
const MONTH: int = 30 * DAY
const YEAR: int = 365 * DAY

@export var target_text: Label
@export var length_box: OptionButton
@export var type_box: OptionButton
@export var reason: TextEdit
@export var root: Control
@export var submit_button: Button

var current_target: UUID


func open(target: UUID) -> void:
	target_text.text = target.to_string()
	reason.text = ""
	root.visible = true
	current_target = target
	await get_tree().create_timer(3).timeout
	submit_button.disabled = false


func submit() -> void:
	submit_button.disabled = true

	if current_target == null:
		return

	var request: Dictionary[String, Variant] = { "target": current_target.to_string() }

	match type_box.selected:
		0:
			request["moderation_type"] = "Ban"
		_:
			push_error("error parsing moderation type")
			submit_button.disabled = false
			return

	var expiry_time: int = 0

	match length_box.selected:
		0:
			expiry_time = Time.get_unix_time_from_system() as int + DAY
		1:
			expiry_time = Time.get_unix_time_from_system() as int + WEEK
		2:
			expiry_time = Time.get_unix_time_from_system() as int + MONTH
		3:
			expiry_time = Time.get_unix_time_from_system() as int + (MONTH * 3)
		4:
			expiry_time = Time.get_unix_time_from_system() as int + (MONTH * 6)
		5:
			expiry_time = Time.get_unix_time_from_system() as int + YEAR
		6:
			expiry_time = Time.get_unix_time_from_system() as int + (YEAR * 3)

	if expiry_time != 0:
		request["expiry_utc"] = expiry_time

	request["reason"] = reason.text

	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		MODERATION_MODERATE_USER_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify(request),
	)

	if response[0] != 200:
		push_error("error submitting moderation")
		submit_button.disabled = false
		return

	root.visible = false
	current_target = null


func cancel() -> void:
	submit_button.disabled = true
	root.visible = false
	current_target = null
