import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/ffi.dart' as rust;

sealed class Fee {
  String get name;
  rust.Fee toAPI();
}

class PriorityFee implements Fee {
  final ConfirmationTarget priority;

  PriorityFee(this.priority);

  @override
  String get name => priority.toString();

  @override
  rust.Fee toAPI() => rust.Fee_Priority(priority.toAPI());
}

class CustomFee implements Fee {
  final Amount amount;

  CustomFee({required this.amount});

  @override
  String get name => "Custom";

  @override
  rust.Fee toAPI() => rust.Fee_Custom(sats: amount.sats);
}
