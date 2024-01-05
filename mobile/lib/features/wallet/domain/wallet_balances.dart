import 'package:get_10101/common/domain/model.dart';

class WalletBalances {
  Amount onChain;
  Amount offChain;

  WalletBalances({required this.onChain, required this.offChain});
}
