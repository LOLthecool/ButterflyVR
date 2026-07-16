extends Node
class_name ObjectEventHandler

#region ActionDefs
@abstract
class BaseAction:
	var active:bool
	var custom_parameters:Array[Variant]
	var id:PackedByteArray
	
	@abstract
	func on_event(parameters:Array[Variant], handler:ObjectEventHandler) -> void
	@abstract
	func init(handler:ObjectEventHandler) -> void


class EnableAction extends BaseAction:
	var take_parameter:bool
	var target_property:TargetProperties
	var target:Node
	
	enum TargetProperties{
		Enable,
		Active
	}
	
	@warning_ignore("shadowed_variable_base_class")
	static func create(id:PackedByteArray, take_parameter:bool, target:Node, 
			target_property:int, active:bool, custom_parameters:Array[Variant]) -> EnableAction:
		var x:EnableAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.take_parameter = take_parameter
		x.target = target
		x.target_property = target_property as TargetProperties
		return x
	
	func on_event(parameters:Array[Variant], _handler:ObjectEventHandler) -> void:
		if !target:
			return
		
		match target_property:
			TargetProperties.Enable:
				if target.get_property_list().map(
						func(x:Dictionary) -> String:
							return x["name"]
				).has("enabled"):
					if take_parameter:
						if parameters.size() > 0 and parameters[0] is bool:
							target.set("enabled", parameters[0])
					else:
						target.set("enabled", !target.get("enabled"))
			
			TargetProperties.Active:
				if target.get_property_list().map(
						func(x:Dictionary) -> String:
							return x["name"]
				).has("active"):
					if take_parameter:
						if parameters.size() > 0 and parameters[0] is bool:
							target.set("active", parameters[0])
					else:
						target.set("active", !target.get("active"))
	
	func init(_handler:ObjectEventHandler) -> void:
		return

class DisplayAction extends BaseAction:
	var text:String
	var target:Label3D
	
	@warning_ignore("shadowed_variable_base_class")
	static func create(id:PackedByteArray, text:String, target:Label3D, active:bool, 
			custom_parameters:Array[Variant]) -> DisplayAction:
		var x:DisplayAction = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.id = id
		x.text = text
		x.target = target
		return x
	
	func on_event(parameters:Array[Variant], _handler:ObjectEventHandler) -> void:
		if !target or !text.contains("%s"):
			return
		target.text = text % (parameters[0] if parameters.size() > 0 else "null")
	
	func init(_handler:ObjectEventHandler) -> void:
		return
#endregion

#region TriggerDefs
@abstract
class BaseTrigger:
	var active:bool
	var custom_parameters:Array[Variant]
	var targets:Array[PackedByteArray]
	
	@abstract
	func init(handler:ObjectEventHandler) -> void
	@abstract
	func tick(handler:ObjectEventHandler) -> void


class AlwaysTrigger extends BaseTrigger:
	var include_tick_count:bool
	var active_ticks:int
	
	@warning_ignore("shadowed_variable_base_class")
	static func create(include_tick_count:bool, active:bool, 
			custom_parameters:Array[Variant], targets:Array[PackedByteArray]) -> AlwaysTrigger:
		var x:AlwaysTrigger = new()
		x.active = active
		x.custom_parameters = custom_parameters
		x.targets = targets
		x.include_tick_count = include_tick_count
		return x
	
	func init(_handler:ObjectEventHandler) -> void:
		return
	
	func tick(handler:ObjectEventHandler) -> void:
		active_ticks += 1
		var parameters:Array[Variant] = custom_parameters.duplicate()
		if include_tick_count:
			parameters.push_back(active_ticks)
		for target:PackedByteArray in targets:
			if !handler.actions.has(target):
				push_error("invalid target %s" % target)
				return
			handler.new_event(handler.actions[target], parameters)

#endregion

class Event:
	var target:BaseAction
	var parameters:Array[Variant]
	
	static func create(target:BaseAction, parameters:Array[Variant]) -> Event:
		var x:Event = new()
		x.target = target
		x.parameters = parameters.duplicate()
		return x
	
	func run(handler:ObjectEventHandler) -> void:
		if target.active:
			parameters.append_array(target.custom_parameters)
			target.on_event(parameters, handler)

var triggers:Array[BaseTrigger] = []
var actions:Dictionary[PackedByteArray, BaseAction] = {}
var active_events:Array[Event] = []
var inactive_events:Array[Event] = []

func register_trigger(trigger:BaseTrigger) -> void:
	triggers.push_back(trigger)
	trigger.init(self)

func register_action(action:BaseAction) -> void:
	if actions.has(action.id):
		push_error("tried to assign duplicate actions with id %s" % action.id)
		return
	actions[action.id] = action
	action.init(self)

func new_event(target:BaseAction, parameters:Array[Variant]) -> void:
	var x:Event = Event.create(target, parameters)
	inactive_events.push_back(x)

func _physics_process(_delta: float) -> void:
	for trigger:BaseTrigger in triggers:
		if trigger.active:
			trigger.tick(self)
	
	for event:Event in active_events:
		event.run(self)
	
	active_events = inactive_events
	inactive_events.clear()
