import 'package:get_10101/common/model.dart';

class Balance {
  final Amount offChain;
  final Amount onChain;

  const Balance(this.offChain, this.onChain);

  factory Balance.fromJson(Map<String, dynamic> json) {
    return switch (json) {
      {
        'on_chain': int onChain,
        'off_chain': int offChain,
      } =>
        Balance(Amount(offChain), Amount(onChain)),
      _ => throw const FormatException('Failed to load balance.'),
    };
  }

  Balance.zero()
      : offChain = Amount.zero(),
        onChain = Amount.zero();

  Amount total() => offChain + onChain;
}
