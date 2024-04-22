import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/channel_configuration.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/util/constants.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

enum TradeAction {
  /// No channel exists.
  openChannel,

  /// Open quantity is bigger than the contracts. The user is partially closing their position.
  reducePosition,

  /// The open quantity is smaller than the contracts. The user is changing directions.
  changeDirection,

  /// The user is either extending or opening a new position. (or changing direction)
  trade,

  /// Open quantity is exactly the amount of contracts. The user is closing their position.
  closePosition,
}

tradeBottomSheetConfirmation(
    {required BuildContext context,
    required Direction direction,
    required TradeAction tradeAction,
    required Function() onConfirmation,
    required ChannelOpeningParams? channelOpeningParams}) {
  final sliderKey = direction == Direction.long
      ? tradeScreenBottomSheetConfirmationSliderBuy
      : tradeScreenBottomSheetConfirmationSliderSell;

  final sliderButtonKey = direction == Direction.long
      ? tradeScreenBottomSheetConfirmationSliderButtonBuy
      : tradeScreenBottomSheetConfirmationSliderButtonSell;

  Amount? fundingTxFee;
  Amount? channelFeeReserve;

  if (tradeAction == TradeAction.openChannel) {
    final DlcChannelService dlcChannelService =
        context.read<DlcChannelChangeNotifier>().dlcChannelService;

    fundingTxFee = dlcChannelService.getEstimatedFundingTxFee();
    channelFeeReserve = dlcChannelService.getEstimatedChannelFeeReserve();
  }

  showModalBottomSheet<void>(
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(
        top: Radius.circular(20),
      ),
    ),
    clipBehavior: Clip.antiAlias,
    isScrollControlled: true,
    useRootNavigator: true,
    context: context,
    barrierColor: Colors.black.withOpacity(TradeAction.closePosition == tradeAction ? 0.4 : 0),
    builder: (BuildContext context) {
      return SafeArea(
        child: Container(
          // decoration: BoxDecoration(border: Border.all(color: Colors.black)),
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          // the GestureDetector ensures that we can close the keyboard by tapping into the modal
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SingleChildScrollView(
              child: SizedBox(
                  height: TradeAction.closePosition == tradeAction ? 380 : 500,
                  child: TradeBottomSheetConfirmation(
                    direction: direction,
                    sliderButtonKey: sliderButtonKey,
                    sliderKey: sliderKey,
                    onConfirmation: onConfirmation,
                    tradeAction: tradeAction,
                    traderCollateral: channelOpeningParams?.traderCollateral,
                    channelFeeReserve: channelFeeReserve,
                    fundingTxFee: fundingTxFee,
                  )),
            ),
          ),
        ),
      );
    },
  );
}

class TradeBottomSheetConfirmation extends StatelessWidget {
  final Direction direction;
  final Key sliderKey;
  final Key sliderButtonKey;
  final Function() onConfirmation;
  final TradeAction tradeAction;

  final Amount? traderCollateral;
  final Amount? channelFeeReserve;
  final Amount? fundingTxFee;

  const TradeBottomSheetConfirmation({
    required this.direction,
    super.key,
    required this.sliderButtonKey,
    required this.sliderKey,
    required this.onConfirmation,
    required this.tradeAction,
    this.traderCollateral,
    this.channelFeeReserve,
    this.fundingTxFee,
  });

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    TradeValues tradeValues =
        Provider.of<TradeValuesChangeNotifier>(context).fromDirection(direction);

    final referralStatus = context.read<TenTenOneConfigChangeNotifier>().referralStatus;

    bool isClose = tradeAction == TradeAction.closePosition;
    bool isChannelOpen = tradeAction == TradeAction.openChannel;

    final traderCollateral1 = traderCollateral ?? Amount.zero();

    Amount reserve = isChannelOpen
        ? (tradeValues.margin?.sats ?? 0) > traderCollateral1.sats
            ? Amount.zero()
            : traderCollateral1.sub(tradeValues.margin ?? Amount.zero())
        : Amount.zero();

    // Fallback to 0 if we can't get the fee or the margin
    Amount total =
        tradeValues.margin != null ? Amount(tradeValues.margin!.sats).add(reserve) : Amount(0);

    Amount pnl = Amount(0);
    if (context.read<PositionChangeNotifier>().positions.containsKey(ContractSymbol.btcusd)) {
      final position = context.read<PositionChangeNotifier>().positions[ContractSymbol.btcusd];
      pnl = position!.unrealizedPnl != null ? position.unrealizedPnl! : Amount(0);
    }

    TextStyle dataRowStyle = const TextStyle(fontSize: 14);

    final orderMatchingFee = tradeValues.fee ?? Amount.zero();
    final feeRebate = referralStatus != null
        ? Amount((referralStatus.referralFeeBonus * orderMatchingFee.sats).floor())
        : Amount.zero();

    final description = switch (tradeAction) {
      TradeAction.closePosition => RichText(
          text: TextSpan(
              text:
                  '\nBy confirming, a market order will be created. Once the order is matched, your position will be closed.',
              style: DefaultTextStyle.of(context).style)),
      TradeAction.openChannel || TradeAction.trade => RichText(
          text: TextSpan(
            text: '\nBy confirming, a market order will be created. Once the order is matched, ',
            style: DefaultTextStyle.of(context).style,
            children: <TextSpan>[
              TextSpan(
                  text: formatSats(total), style: const TextStyle(fontWeight: FontWeight.bold)),
              const TextSpan(text: ' will be locked up in a DLC channel!'),
            ],
          ),
        ),
      TradeAction.reducePosition => RichText(
          text: TextSpan(
            text: '\nBy confirming, a market order will be created reducing your position to ',
            style: DefaultTextStyle.of(context).style,
            children: <TextSpan>[
              TextSpan(
                  text: formatUsd(tradeValues.openQuantity - tradeValues.contracts),
                  style: const TextStyle(fontWeight: FontWeight.bold)),
            ],
          ),
        ),
      TradeAction.changeDirection => RichText(
          text: TextSpan(
            text:
                '\nBy confirming, a market order will be created changing the direction of your position to ',
            style: DefaultTextStyle.of(context).style,
            children: <TextSpan>[
              TextSpan(
                  text: formatUsd(tradeValues.contracts - tradeValues.openQuantity),
                  style: const TextStyle(fontWeight: FontWeight.bold)),
              TextSpan(text: ' ${tradeValues.direction.nameU}.\n\nOnce the order is matched, '),
              TextSpan(
                  text: formatSats(total), style: const TextStyle(fontWeight: FontWeight.bold)),
              const TextSpan(text: ' will be locked up in a DLC channel!')
            ],
          ),
        ),
    };

    return Container(
        padding: EdgeInsets.only(left: 20, right: 20, top: (isClose ? 20 : 10), bottom: 10),
        child: Column(
          children: [
            const ContractSymbolIcon(),
            Text("Market ${direction.nameU}",
                style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17, color: color)),
            Center(
              child: Container(
                padding: const EdgeInsets.symmetric(vertical: 10),
                child: Column(
                  children: [
                    Wrap(
                      runSpacing: 5,
                      children: [
                        ValueDataRow(
                            type: ValueType.fiat,
                            value: tradeValues.contracts.asDouble(),
                            label: "Quantity"),
                        if (!isClose)
                          ValueDataRow(
                              type: ValueType.date,
                              value: tradeValues.expiry.toLocal(),
                              label: 'Expiry'),
                        isClose
                            ? ValueDataRow(
                                type: ValueType.fiat,
                                value: tradeValues.price ?? 0.0,
                                label: 'Market Price')
                            : ValueDataRow(
                                type: ValueType.amount, value: tradeValues.margin, label: 'Margin'),
                        isClose
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: pnl,
                                label: 'Unrealized P/L',
                                valueTextStyle: dataRowStyle.apply(
                                    color:
                                        pnl.sats.isNegative ? tradeTheme.loss : tradeTheme.profit))
                            : ValueDataRow(
                                type: ValueType.fiat,
                                value: tradeValues.liquidationPrice ?? 0.0,
                                label: 'Liquidation Price',
                              ),
                        ValueDataRow(
                          type: ValueType.amount,
                          value: orderMatchingFee,
                          label: "Order-matching fee",
                        ),
                        if (referralStatus != null && referralStatus.referralFeeBonus > 0)
                          Row(
                              mainAxisSize: MainAxisSize.max,
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Text(
                                    "Fee rebate (${(referralStatus.referralFeeBonus * 100.0).toStringAsFixed(0)}%)",
                                    style: const TextStyle(color: Colors.green)),
                                Text(
                                  "-$feeRebate",
                                  style: const TextStyle(color: Colors.green),
                                )
                              ]),
                        isChannelOpen
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: reserve,
                                label: 'Channel collateral reserve')
                            : const SizedBox(height: 0),
                        isChannelOpen
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: channelFeeReserve,
                                label: 'Channel fee reserve')
                            : const SizedBox(height: 0),
                        isChannelOpen
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: openingFee,
                                label: 'Channel-opening fee',
                              )
                            : const SizedBox(height: 0),
                        isChannelOpen
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: fundingTxFee,
                                label: 'Transaction fee estimate',
                              )
                            : const SizedBox(height: 0),
                      ],
                    ),
                    !isClose ? const Divider() : const SizedBox(height: 0),
                    !isClose
                        ? ValueDataRow(type: ValueType.amount, value: total, label: "Total")
                        : const SizedBox(height: 0),
                  ],
                ),
              ),
            ),
            description,
            const Spacer(),
            ConfirmationSlider(
              key: sliderKey,
              text: "Swipe to confirm ${direction.nameU}",
              textStyle: TextStyle(color: color),
              height: 40,
              foregroundColor: color,
              sliderButtonContent: Container(
                key: sliderButtonKey,
                child: const Icon(
                  Icons.chevron_right,
                  color: Colors.white,
                  size: 20,
                ),
              ),
              onConfirmation: onConfirmation,
            )
          ],
        ));
  }
}
