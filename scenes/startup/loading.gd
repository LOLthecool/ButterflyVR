extends VBoxContainer
class_name LoadingMenu
# todo: use when signing up / logging in

@export var greeter:GreeterMenu
@export var loading_text:Label

@export var root:Node

var canceled:bool = false

func _init() -> void:
	if OS.get_cmdline_args().has("--server") or OS.get_cmdline_args().has("--headless"):
		# todo: awaiting this call breaks things and im not sure why
		@warning_ignore("missing_await")
		ServerLoader.start()

func _ready() -> void:
	if OS.get_cmdline_args().has("--server") or OS.get_cmdline_args().has("--headless"):
		await root.ready
		root.queue_free()
		return

	if await GlobalAccountHandler.check_token_valid():
		loading_text.text = "Logging in with saved account..."
		await start()
	else:
		greeter.visible = true

func start() -> void:
	visible = true
	loading_text.text = "Loading homeworld..."
	
	var t1:int = Time.get_ticks_msec()
	var homeworld:UUID = GlobalPersistanceHandler.register_value("cache", "preload_objects", "cached_homeworld", null)
	if homeworld:
		# preload would just guarentee object is in cache, this loads the resource pack too
		# todo: method for loading resource pack (split resourceloader call to own function?)
		await GlobalDownloadHandler.get_object(homeworld, TypeHelper.ObjectType.world)
	var wait_length:float = maxf(0, 1000 - (Time.get_ticks_msec() - t1)) / 1000
	
	
	await get_tree().create_timer(wait_length).timeout # give user a chance to cancel load
	if canceled:
		canceled = false
		@warning_ignore("unsafe_property_access")
		greeter.visible = true
	else:
		await GlobalWorldHandler.load_homeworld()


func _on_button_pressed() -> void:
	canceled = true
