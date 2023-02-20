import 'package:get_10101/common/domain/model.dart';

class WalletBalances {
  Amount onChain;
  Amount lightning;

  WalletBalances({required this.onChain, required this.lightning});
}
