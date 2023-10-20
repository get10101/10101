import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/balance_row.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class Balance extends StatelessWidget {
  const Balance({super.key});

  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();
    Amount total = walletChangeNotifier.total();
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();
    final position = positionChangeNotifier.positions[ContractSymbol.btcusd];
    if (position != null) {
      total = total.add(position.getAmountWithUnrealizedPnl());
    }

    var (leading, balance) = getFormattedBalance(total.toInt);

    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        maintainState: true,
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(leading,
                style: const TextStyle(
                  color: Colors.grey,
                  fontSize: 28.0,
                  fontWeight: FontWeight.bold,
                )),
            Text(balance,
                style: const TextStyle(
                  color: Colors.black87,
                  fontSize: 28.0,
                  fontWeight: FontWeight.bold,
                )),
            const Icon(Icons.currency_bitcoin, size: 28, color: tenTenOnePurple),
            const SizedBox(width: 16)
          ],
        ),
        controlAffinity: ListTileControlAffinity.leading,
        children: const [
          Card(
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.vertical(
                  top: Radius.circular(5),
                  bottom: Radius.circular(5),
                ),
              ),
              child: Padding(
                padding: EdgeInsets.only(top: 4.0, bottom: 4.0, right: 8.0),
                child: Column(children: [
                  BalanceRow(walletType: WalletType.lightning),
                  Divider(
                      height: 2, thickness: 1, indent: 10, endIndent: 10, color: Colors.black12),
                  BalanceRow(walletType: WalletType.onChain),
                  Divider(
                      height: 2, thickness: 1, indent: 10, endIndent: 10, color: Colors.black12),
                  BalanceRow(walletType: WalletType.stable),
                ]),
              ))
        ],
      ),
    );
  }
}
