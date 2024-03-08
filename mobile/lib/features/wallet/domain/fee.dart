import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/ffi.dart' as rust;

sealed class FeeConfig {
  String get name;
  rust.FeeConfig toAPI();
}

class PriorityFee implements FeeConfig {
  final ConfirmationTarget priority;

  PriorityFee(this.priority);

  @override
  String get name => priority.toString();

  @override
  rust.FeeConfig toAPI() => rust.FeeConfig_Priority(priority.toAPI());
}

class CustomFeeRate implements FeeConfig {
  final int feeRate;

  CustomFeeRate({required this.feeRate});

  @override
  String get name => "Custom fee rate";

  @override
  rust.FeeConfig toAPI() => rust.FeeConfig_FeeRate(satsPerVbyte: feeRate.toDouble());
}
