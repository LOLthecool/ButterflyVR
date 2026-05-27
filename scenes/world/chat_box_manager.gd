extends MessageHandler
class_name ChatBoxManager

signal new_message_sent(message:Message)

var messages:Array[Message]

class Message:
	var player:PackedByteArray
	var text:String


func send_message(message_text:String) -> void:
	var player:PackedByteArray = (await GlobalAccountHandler.get_uuid()).backing_storage
	send_message_final([player, message_text], [_get_value_type(null, 0), _get_value_type(player, 1)])

func _get_value_type(_previous_value: Variant, idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.ByteArray
		1:
			return TypeHelper.NetworkedValueTypes.String
	return TypeHelper.NetworkedValueTypes.End

func _process_message(values: Array) -> void:
	var message:Message = Message.new()
	message.player = values[0]
	message.text = values[1]
	new_message_sent.emit(message)
	messages.append(message)
