import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/change_notifier/trade_constraint_change_notifier.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/calculations.dart';
import 'package:get_10101/common/contract_symbol_icon.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/theme.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/services/new_order_service.dart';
import 'package:get_10101/services/quote_service.dart';
import 'package:get_10101/services/trade_constraints_service.dart';
import 'package:get_10101/trade/collateral_slider.dart';
import 'package:provider/provider.dart';

class CreateChannelConfirmationDialog extends StatefulWidget {
  final Direction direction;
  final Function() onConfirmation;
  final Function() onCancel;
  final BestQuote bestQuote;
  final Amount? fee;
  final Amount margin;
  final Leverage leverage;
  final Usd quantity;

  const CreateChannelConfirmationDialog(
      {super.key,
      required this.direction,
      required this.onConfirmation,
      required this.onCancel,
      required this.bestQuote,
      required this.fee,
      required this.leverage,
      required this.quantity,
      required this.margin});

  @override
  State<CreateChannelConfirmationDialog> createState() => _CreateChannelConfirmationDialogState();
}

class _CreateChannelConfirmationDialogState extends State<CreateChannelConfirmationDialog> {
  final TextEditingController _ownCollateralController = TextEditingController();

  // TODO: Once we have one, fetch it from backend
  Amount openingFee = Amount(0);

  final _formKey = GlobalKey<FormState>();
  Amount _ownChannelCollateral = Amount.zero();
  Amount _counterpartyChannelCollateral = Amount.zero();

  @override
  void initState() {
    super.initState();

    TradeConstraintsChangeNotifier changeNotifier = context.read<TradeConstraintsChangeNotifier>();

    _ownChannelCollateral = widget.margin;

    changeNotifier.service.getTradeConstraints().then((value) {
      setState(() {
        _ownChannelCollateral = Amount(max(widget.margin.sats, value.minMarginSats));
        var coordinatorLeverage = value.coordinatorLeverage;
        var counterpartyMargin = calculateMargin(widget.quantity, widget.bestQuote,
            Leverage(coordinatorLeverage), widget.direction == Direction.short);
        updateCounterpartyCollateral(counterpartyMargin, coordinatorLeverage);
        _ownCollateralController.text = _ownChannelCollateral.formatted();
      });
    });
  }

  @override
  void dispose() {
    _ownCollateralController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    TradeConstraintsChangeNotifier changeNotifier = context.watch<TradeConstraintsChangeNotifier>();

    final messenger = ScaffoldMessenger.of(context);

    var tradeConstraints = changeNotifier.tradeConstraints;

    return Dialog(
        insetPadding: const EdgeInsets.all(15),
        shape: const RoundedRectangleBorder(borderRadius: BorderRadius.all(Radius.circular(10.0))),
        child: SingleChildScrollView(
          child: tradeConstraints == null
              ? const CircularProgressIndicator()
              : createDialogContent(tradeConstraints, context, messenger),
        ));
  }

  Widget createDialogContent(
      TradeConstraints tradeConstraints, BuildContext context, ScaffoldMessengerState messenger) {
    double coordinatorLeverage = tradeConstraints.coordinatorLeverage;
    Amount counterpartyMargin = calculateMargin(widget.quantity, widget.bestQuote,
        Leverage(coordinatorLeverage), widget.direction == Direction.short);

    final maxCounterpartyCollateral = Amount(tradeConstraints.maxCounterpartyMarginSats);

    final maxOnChainSpending = Amount(tradeConstraints.maxLocalMarginSats);
    final counterpartyLeverage = tradeConstraints.coordinatorLeverage;

    final minMargin = Amount(max(tradeConstraints.minMarginSats, widget.margin.sats));

    final orderMatchingFees = widget.fee ?? Amount.zero();

    var estimatedFundingTxFeeSats = Amount(tradeConstraints.estimatedFundingTxFeeSats);
    final Amount fundingTxFeeWithBuffer = Amount(estimatedFundingTxFeeSats.sats * 2);

    var channelFeeReserve = Amount(tradeConstraints.channelFeeReserveSats);
    final maxUsableOnChainBalance = Amount.max(
        maxOnChainSpending - orderMatchingFees - fundingTxFeeWithBuffer - channelFeeReserve,
        Amount.zero());
    final maxCounterpartyCollateralSats =
        (maxCounterpartyCollateral.sats * counterpartyLeverage).toInt();

    final int collateralSliderMaxValue =
        min(maxCounterpartyCollateralSats, maxUsableOnChainBalance.toInt);

    // if we don't have enough on-chain balance to pay for min margin, we set it to `minMargin` nevertheless and disable the slider
    // this could be the case if we do not have enough money.
    final int collateralSliderMinValue = minMargin.sats;
    bool notEnoughOnchainBalance = false;
    if (maxUsableOnChainBalance.sats < minMargin.sats) {
      notEnoughOnchainBalance = true;
    }

    return Form(
      key: _formKey,
      child: Padding(
        padding: const EdgeInsets.all(8.0),
        child: SizedBox(
          width: 450,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    children: [
                      const ContractSymbolIcon(),
                      const Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Text("DLC Channel Configuration",
                              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17)),
                        ],
                      ),
                      const SizedBox(height: 20),
                      Text(
                          "This is your first trade which will open a DLC Channel and opens your position.",
                          style: DefaultTextStyle.of(context).style),
                      const SizedBox(height: 10),
                      Text(
                          "Specify your preferred channel size, impacting how much you will be able to win up to.",
                          style: DefaultTextStyle.of(context).style),
                      const SizedBox(height: 20),
                      CollateralSlider(
                        onValueChanged: notEnoughOnchainBalance
                            ? null
                            : (newValue) {
                                var parsedAmount = Amount(newValue);
                                _ownCollateralController.text = parsedAmount.formatted();
                                setState(() {
                                  _ownChannelCollateral = parsedAmount;
                                  updateCounterpartyCollateral(
                                      counterpartyMargin, coordinatorLeverage);
                                });
                              },
                        minValue: collateralSliderMinValue,
                        maxValue: collateralSliderMaxValue,
                        labelText: 'Your collateral (sats)',
                        value: _ownChannelCollateral.sats,
                      ),
                      const SizedBox(height: 15),
                      AmountInputField(
                        enabled: !notEnoughOnchainBalance,
                        controller: _ownCollateralController,
                        label: 'Your collateral (sats)',
                        heightPadding: 40,
                        widthPadding: 20,
                        onChanged: (value) {
                          setState(() {
                            _ownChannelCollateral = Amount.parseAmount(value);
                            updateCounterpartyCollateral(counterpartyMargin, coordinatorLeverage);
                            _formKey.currentState!.validate();
                          });
                        },
                        suffixIcon: TextButton(
                          onPressed: () {
                            setState(() {
                              _ownChannelCollateral = Amount(
                                  min(maxCounterpartyCollateralSats, maxUsableOnChainBalance.sats));
                              _ownCollateralController.text = _ownChannelCollateral.formatted();

                              updateCounterpartyCollateral(counterpartyMargin, coordinatorLeverage);
                            });
                          },
                          child: const Text(
                            "Max",
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                        validator: (value) {
                          if (_ownChannelCollateral.sats < minMargin.sats) {
                            return "Min collateral: $minMargin";
                          }

                          if ((_ownChannelCollateral + orderMatchingFees).sats >
                              maxOnChainSpending.sats) {
                            return "Max on-chain: $maxUsableOnChainBalance";
                          }

                          if (maxCounterpartyCollateral.sats < counterpartyMargin.sats) {
                            return "Over limit: $maxCounterpartyCollateral";
                          }

                          return null;
                        },
                      ),
                      const SizedBox(height: 15),
                      AmountTextField(
                        value: Amount(min(
                            maxCounterpartyCollateralSats, _counterpartyChannelCollateral.sats)),
                        label: 'Win up to (sats)',
                      ),
                      const SizedBox(
                        height: 15,
                      ),
                      ValueDataRow(
                          type: ValueType.amount,
                          value: _ownChannelCollateral,
                          label: 'Your collateral'),
                      ValueDataRow(
                        type: ValueType.amount,
                        value: openingFee,
                        label: 'Channel-opening fee',
                      ),
                      ValueDataRow(
                        type: ValueType.amount,
                        value: widget.fee,
                        label: 'Order matching fee',
                      ),
                      ValueDataRow(
                        type: ValueType.amount,
                        value: estimatedFundingTxFeeSats,
                        label: 'Blockchain transaction fee estimate',
                      ),
                      ValueDataRow(
                        type: ValueType.amount,
                        value: channelFeeReserve,
                        label: 'Channel transaction fee reserve',
                      ),
                      const Divider(),
                      ValueDataRow(
                          type: ValueType.amount,
                          value: _ownChannelCollateral +
                              widget.fee! +
                              estimatedFundingTxFeeSats +
                              channelFeeReserve,
                          label: "Total"),
                      const SizedBox(height: 10),
                      Padding(
                        padding: const EdgeInsets.only(top: 20.0),
                        child: Visibility(
                          visible: notEnoughOnchainBalance,
                          replacement: RichText(
                              textAlign: TextAlign.justify,
                              text: TextSpan(
                                  text:
                                      'By confirming, a market order will be created. Once the order is matched your channel will be opened and your position will be created.',
                                  style: DefaultTextStyle.of(context).style)),
                          child: RichText(
                              textAlign: TextAlign.justify,
                              text: const TextSpan(
                                  text:
                                      'You do not have enough balance in your on-chain wallet. Please fund it with at least 270,000 sats.',
                                  style: TextStyle(color: TenTenOneTheme.red600))),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 20.0),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.center,
                          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                          children: [
                            ElevatedButton(
                              onPressed: () {
                                widget.onCancel();
                                Navigator.pop(context);
                              },
                              style: ElevatedButton.styleFrom(
                                  backgroundColor: Colors.grey, fixedSize: const Size(100, 20)),
                              child: const Text('Cancel'),
                            ),
                            ElevatedButton(
                              onPressed: notEnoughOnchainBalance
                                  ? null
                                  : () async {
                                      await NewOrderService.postNewOrder(widget.leverage,
                                              widget.quantity, widget.direction == Direction.long,
                                              channelOpeningParams: ChannelOpeningParams(
                                                  Amount.max(
                                                      Amount.zero(),
                                                      (_counterpartyChannelCollateral -
                                                          counterpartyMargin)),
                                                  Amount.max(Amount.zero(),
                                                      _ownChannelCollateral - widget.margin)))
                                          .then((orderId) {
                                        showSnackBar(
                                            messenger, "Market order created. Order id: $orderId.");
                                        Navigator.pop(context);
                                      }).catchError((error) {
                                        showSnackBar(
                                            messenger, "Failed creating market order: $error.");
                                      }).whenComplete(widget.onConfirmation);
                                    },
                              style: ElevatedButton.styleFrom(fixedSize: const Size(100, 20)),
                              child: const Text('Accept'),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ))
            ],
          ),
        ),
      ),
    );
  }

  void updateCounterpartyCollateral(Amount counterpartyMargin, double counterpartyLeverage) {
    final collateral = (_ownChannelCollateral.sats.toDouble() / counterpartyLeverage).floor();
    _counterpartyChannelCollateral = Amount(collateral.toInt());
  }
}
