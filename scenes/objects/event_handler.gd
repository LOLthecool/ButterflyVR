extends Node
class_name ObjectEventHandler

#region ActionDefs

@abstract
class BaseAction:
	var active: bool
	var custom_parameters: Array[Variant]
	var id: PackedByteArray


	@abstract func on_event(parameters: Array[Variant], handler: ObjectEventHandler) -> void


	@abstract func init(handler: ObjectEventHandler) -> void


class EnableAction extends BaseAction:
	var take_parameter: bool
	var target_property: TargetProperties
	var target: Node

	enum TargetProperties {
		Enable,
		Active,
	}


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		id: PackedByteArray,
		take_parameter: bool,
		target: Node,
		target_property: int,
		active: bool,
		custom_parameters: Array[Variant],
	) -> EnableAction:
		var x: EnableAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.take_parameter = take_parameter
		x.target = target
		x.target_property = target_property as TargetProperties
		return x


	func on_event(parameters: Array[Variant], _handler: ObjectEventHandler) -> void:
		if !target:
			return

		match target_property:
			TargetProperties.Enable:
				if target \
						.get_property_list() \
						.map(
					func(x: Dictionary) -> String:
						return x["name"],
				) \
						.has("enabled"):
					if take_parameter:
						if parameters.size() > 0 and parameters[0] is bool:
							target.set("enabled", parameters[0])
					else:
						target.set("enabled", !target.get("enabled"))

			TargetProperties.Active:
				if target \
						.get_property_list() \
						.map(
					func(x: Dictionary) -> String:
						return x["name"],
				) \
						.has("active"):
					if take_parameter:
						if parameters.size() > 0 and parameters[0] is bool:
							target.set("active", parameters[0])
					else:
						target.set("active", !target.get("active"))


	func init(_handler: ObjectEventHandler) -> void:
		return


class DisplayAction extends BaseAction:
	var text: String
	var target: Label3D


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		id: PackedByteArray,
		text: String,
		target: Label3D,
		active: bool,
		custom_parameters: Array[Variant],
	) -> DisplayAction:
		var x: DisplayAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.text = text
		x.target = target
		return x


	func on_event(parameters: Array[Variant], _handler: ObjectEventHandler) -> void:
		if !target or !text.contains("%s"):
			return
		target.text = text % (parameters[0] if parameters.size() > 0 else "null")


	func init(_handler: ObjectEventHandler) -> void:
		return


class ParameterAction extends BaseAction:
	var parameter: String
	var target: AnimationTree


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		id: PackedByteArray,
		target: AnimationTree,
		parameter: String,
		active: bool,
		custom_parameters: Array[Variant],
	) -> ParameterAction:
		var x: ParameterAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.target = target
		x.parameter = parameter
		return x


	func on_event(parameters: Array[Variant], _handler: ObjectEventHandler) -> void:
		if parameters.size() > 0:
			target.set("parameters/" + parameter, parameters[0])


	func init(_handler: ObjectEventHandler) -> void:
		return


class TransitionAction extends BaseAction:
	var target: AnimationTree
	var state_machine: String
	var target_node: String
	var source_node: String
	var teleport: bool


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		id: PackedByteArray,
		target: AnimationTree,
		state_machine: String,
		target_node: String,
		source_node: String,
		teleport: bool,
		active: bool,
		custom_parameters: Array[Variant],
	) -> TransitionAction:
		var x: TransitionAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.target = target
		x.state_machine = state_machine
		x.target_node = target_node
		x.source_node = source_node
		x.teleport = teleport
		return x


	func on_event(_parameters: Array[Variant], _handler: ObjectEventHandler) -> void:
		var state_machine_playback: AnimationNodeStateMachinePlayback
		if state_machine == "ROOT":
			state_machine_playback = target.get("parameters/playback")
		else:
			var path: String = "parameters/"
			for chunk: String in state_machine.trim_prefix("ROOT/").split("/"):
				path += chunk.substr(1)
			state_machine_playback = target.get(path + "/playback")

		if teleport:
			if source_node == "None" or source_node == state_machine_playback.get_current_node():
				state_machine_playback.start(target_node)
		else:
			if source_node == "None" or source_node == state_machine_playback.get_current_node():
				state_machine_playback.travel(target_node)


	func init(_handler: ObjectEventHandler) -> void:
		return


class AnimationSetAction extends BaseAction:
	var from_parameter: bool
	var animation: String
	var target: AnimationPlayer
	var path_start_point: Node
	var target_path: String


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		id: PackedByteArray,
		animation: String,
		target_path: String,
		path_start_point: Node,
		active: bool,
		custom_parameters: Array[Variant],
	) -> AnimationSetAction:
		var x: AnimationSetAction = new()
		x.id = id
		x.active = active
		x.custom_parameters = custom_parameters
		x.from_parameter = animation == "From Parameter"
		x.animation = animation
		x.path_start_point = path_start_point
		x.target_path = target_path
		return x


	func on_event(parameters: Array[Variant], _handler: ObjectEventHandler) -> void:
		if !from_parameter:
			@warning_ignore("unsafe_cast")
			target.play(
				animation,
				-1,
				(
					parameters[0] as float
					if parameters.size() > 0 and parameters[0] is float
					else target.speed_scale
				),
			)
		else:
			if parameters.size() > 0 and parameters[0] is String:
				@warning_ignore("unsafe_cast")
				target.play(
					parameters[0] as String,
					-1,
					(
						parameters[1] as float
						if parameters.size() > 1 and parameters[1] is float
						else target.speed_scale
					),
				)
			else:
				target.stop()


	func init(_handler: ObjectEventHandler) -> void:
		target = path_start_point.get_node(target_path)
		path_start_point = null
		target_path = ""
#endregion

#region TriggerDefs

@abstract
class BaseTrigger:
	var active: bool
	var custom_parameters: Array[Variant]
	var targets: Array[PackedByteArray]


	@abstract func init(handler: ObjectEventHandler) -> void


	@abstract func tick(handler: ObjectEventHandler) -> void


class AlwaysTrigger extends BaseTrigger:
	var include_tick_count: bool
	var active_ticks: int


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		include_tick_count: bool,
		active: bool,
		custom_parameters: Array[Variant],
		targets: Array[PackedByteArray],
	) -> AlwaysTrigger:
		var x: AlwaysTrigger = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.targets = targets
		x.include_tick_count = include_tick_count
		return x


	func init(_handler: ObjectEventHandler) -> void:
		return


	func tick(handler: ObjectEventHandler) -> void:
		active_ticks += 1
		var parameters: Array[Variant] = custom_parameters.duplicate()
		if include_tick_count:
			parameters.push_back(active_ticks)
		for target: PackedByteArray in targets:
			if !handler.actions.has(target):
				push_error("invalid target %s" % target)
				return
			handler.new_event(handler.actions[target], parameters)


class JumpTrigger extends BaseTrigger:
	var player: Player
	var was_on_floor: bool = false


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		player: Player,
		active: bool,
		custom_parameters: Array[Variant],
		targets: Array[PackedByteArray],
	) -> JumpTrigger:
		var x: JumpTrigger = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.targets = targets
		x.player = player
		return x


	func init(_handler: ObjectEventHandler) -> void:
		return


	func tick(handler: ObjectEventHandler) -> void:
		if was_on_floor and !player.is_on_floor():
			var parameters: Array[Variant] = custom_parameters.duplicate()
			for target: PackedByteArray in targets:
				if !handler.actions.has(target):
					push_error("invalid target %s" % target)
				else:
					handler.new_event(handler.actions[target], parameters)
		was_on_floor = player.is_on_floor()


class LandTrigger extends BaseTrigger:
	var player: Player
	var was_on_floor: bool = true


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		player: Player,
		active: bool,
		custom_parameters: Array[Variant],
		targets: Array[PackedByteArray],
	) -> LandTrigger:
		var x: LandTrigger = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.targets = targets
		x.player = player
		return x


	func init(_handler: ObjectEventHandler) -> void:
		return


	func tick(handler: ObjectEventHandler) -> void:
		if !was_on_floor and player.is_on_floor():
			var parameters: Array[Variant] = custom_parameters.duplicate()
			for target: PackedByteArray in targets:
				if !handler.actions.has(target):
					push_error("invalid target %s" % target)
				else:
					handler.new_event(handler.actions[target], parameters)
		was_on_floor = player.is_on_floor()


class VelocityTrigger extends BaseTrigger:
	var player: Player
	var last_velocity: Vector2


	@warning_ignore("shadowed_variable_base_class")
	static func create(
		player: Player,
		active: bool,
		custom_parameters: Array[Variant],
		targets: Array[PackedByteArray],
	) -> VelocityTrigger:
		var x: VelocityTrigger = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.targets = targets
		x.player = player
		return x


	func init(_handler: ObjectEventHandler) -> void:
		return


	func tick(handler: ObjectEventHandler) -> void:
		var parameters: Array[Variant] = custom_parameters.duplicate()
		var absolute_velocity: Vector2 = Vector2(player.velocity.x, player.velocity.z)

		if absolute_velocity == last_velocity:
			return

		last_velocity = absolute_velocity

		parameters.push_back(absolute_velocity.rotated(player.rotation.y))
		for target: PackedByteArray in targets:
			if !handler.actions.has(target):
				push_error("invalid target %s" % target)
				return
			handler.new_event(handler.actions[target], parameters)

#endregion

class Event:
	var target: BaseAction
	var parameters: Array[Variant]


	static func create(target: BaseAction, parameters: Array[Variant]) -> Event:
		var x: Event = new()
		x.target = target
		x.parameters = parameters.duplicate()
		return x


	func run(handler: ObjectEventHandler) -> void:
		if target.active:
			parameters.append_array(target.custom_parameters)
			target.on_event(parameters, handler)


var triggers: Array[BaseTrigger] = []
var actions: Dictionary[PackedByteArray, BaseAction] = { }
var active_events: Array[Event] = []
var inactive_events: Array[Event] = []


func register_trigger(trigger: BaseTrigger) -> void:
	if triggers.size() > 256:
		push_error("too many triggers registered for object %s" % get_parent().name)
		return

	triggers.push_back(trigger)
	trigger.init(self)


func register_action(action: BaseAction) -> void:
	if actions.size() > 256:
		push_error("too many actions registered for object %s" % get_parent().name)
		return

	if actions.has(action.id):
		push_error("tried to assign duplicate actions with id %s" % action.id)
		return
	actions[action.id] = action
	await GlobalTreeAccessHelper.physics_frame
	action.init(self)


func new_event(target: BaseAction, parameters: Array[Variant]) -> void:
	if inactive_events.size() > 64:
		push_error("too many scheduled events for object %s" % get_parent().name)
		return

	var x: Event = Event.create(target, parameters)
	inactive_events.push_back(x)


func _physics_process(_delta: float) -> void:
	for trigger: BaseTrigger in triggers:
		if trigger.active:
			trigger.tick(self)

	for event: Event in active_events:
		event.run(self)

	active_events = inactive_events
	inactive_events.clear()
