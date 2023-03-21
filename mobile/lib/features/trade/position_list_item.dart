import 'package:expandable/expandable.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

import 'contract_symbol_icon.dart';

class PositionListItem extends StatelessWidget {
  const PositionListItem({super.key, required this.position, required this.onClose});

  final Position? position;
  final Function onClose;

  @override
  Widget build(BuildContext context) {
    if (position == null) {
      return const NoPositionsListItem();
    }

    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    // dart cannot promote...
    Position notNullPosition = position!;
    TextStyle dataRowStyle = const TextStyle(fontSize: 16);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(10.0),
        child: ExpandablePanel(
          theme: const ExpandableThemeData(
            hasIcon: false,
            tapBodyToExpand: true,
            tapHeaderToExpand: true,
          ),
          header: Column(
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const ContractSymbolIcon(),
                      const SizedBox(
                        width: 10,
                      ),
                      Text(
                        notNullPosition.contractSymbol.label,
                        style: const TextStyle(fontWeight: FontWeight.bold),
                      ),
                    ],
                  ),
                  Chip(
                    label: Text(notNullPosition.positionState.name),
                    backgroundColor: Colors.transparent,
                    shape: const StadiumBorder(side: BorderSide()),
                  )
                ],
              ),
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 10),
                child: Wrap(
                  runSpacing: 10,
                  children: [
                    notNullPosition.unrealizedPnl == null
                        ? const ValueDataRow(
                            type: ValueType.loading, value: "", label: "Unrealized P/L")
                        : ValueDataRow(
                            type: ValueType.amount,
                            value: notNullPosition.unrealizedPnl,
                            label: "Unrealized P/L",
                            valueTextStyle: dataRowStyle.apply(
                                color: notNullPosition.unrealizedPnl!.sats.isNegative
                                    ? tradeTheme.loss
                                    : tradeTheme.profit),
                            labelTextStyle: dataRowStyle,
                          ),
                    ValueDataRow(
                      type: ValueType.amount,
                      value: notNullPosition.collateral,
                      label: "Margin",
                      valueTextStyle: dataRowStyle,
                      labelTextStyle: dataRowStyle,
                    ),
                    ValueDataRow(
                      type: ValueType.contracts,
                      value: notNullPosition.quantity,
                      label: "Quantity",
                      valueTextStyle: dataRowStyle,
                      labelTextStyle: dataRowStyle,
                    ),
                    ValueDataRow(
                      type: ValueType.fiat,
                      value: notNullPosition.liquidationPrice,
                      label: "Liquidation price",
                      valueTextStyle: dataRowStyle,
                      labelTextStyle: dataRowStyle,
                    ),
                    ValueDataRow(
                      type: ValueType.fiat,
                      value: notNullPosition.averageEntryPrice,
                      label: "Average entry price",
                      valueTextStyle: dataRowStyle,
                      labelTextStyle: dataRowStyle,
                    ),
                  ],
                ),
              ),
            ],
          ),
          collapsed: Container(),
          expanded: Row(
            mainAxisAlignment: MainAxisAlignment.end,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              OutlinedButton(
                onPressed: () {
                  // TODO: Navigate to details child screen (that also includes close button at the bottom)
                },
                child: const Text("Show Details"),
              ),
              const SizedBox(
                width: 10,
              ),
              ElevatedButton(
                onPressed: notNullPosition.positionState == PositionState.closing
                    ? null
                    : () async {
                        await onClose();
                      },
                child: notNullPosition.positionState == PositionState.closing
                    ? Row(
                        children: const [
                          SizedBox(
                            width: 10,
                            height: 10,
                            child: CircularProgressIndicator(),
                          ),
                          Text("Closing ...")
                        ],
                      )
                    : const Text("Close Position"),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class NoPositionsListItem extends StatelessWidget {
  const NoPositionsListItem({super.key});

  @override
  Widget build(BuildContext context) {
    return const Card(
      child: ListTile(
        leading: ContractSymbolIcon(),
        title: Text("You don't have any position yet..."),
        subtitle: Text("Trade now to open a position!"),
      ),
    );
  }
}
