import 'package:get_10101/common/amount.dart';

class Balance {
  final Amount offChain;
  final Amount onChain;

  const Balance(this.offChain, this.onChain);

  Balance.zero()
      : offChain = Amount.zero(),
        onChain = Amount.zero();

  Amount total() => offChain.add(onChain);
}
