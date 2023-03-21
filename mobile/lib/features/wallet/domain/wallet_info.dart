import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;
import 'package:get_10101/common/domain/model.dart';
import 'wallet_history.dart';
import 'wallet_balances.dart';

class WalletInfo {
  WalletBalances balances;
  List<WalletHistoryItemData> history;

  WalletInfo({required this.balances, required this.history});
  WalletInfo.fromApi(rust.WalletInfo walletInfo)
      : balances = WalletBalances(
            onChain: Amount(walletInfo.balances.onChain),
            lightning: Amount(walletInfo.balances.lightning)),
        history = walletInfo.history.map((item) {
          return WalletHistoryItemData.fromApi(item);
        }).toList();

  static rust.WalletInfo apiDummy() {
    return rust.WalletInfo(
      balances: const rust.Balances(onChain: -1, lightning: -1),
      history: List.empty(growable: false),
    );
  }
}
