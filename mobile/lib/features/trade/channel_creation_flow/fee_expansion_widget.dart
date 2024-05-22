import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';

class FeeExpansionTile extends StatefulWidget {
  final Amount value;
  final Amount orderMatchingFee;
  final Amount fundingTxFee;
  final Amount channelFeeReserve;

  const FeeExpansionTile({
    super.key,
    required this.value,
    required this.orderMatchingFee,
    required this.fundingTxFee,
    required this.channelFeeReserve,
  });

  @override
  State<FeeExpansionTile> createState() => _FeeExpansionTileState();
}

class _FeeExpansionTileState extends State<FeeExpansionTile> with SingleTickerProviderStateMixin {
  bool _isExpanded = false;
  late AnimationController _controller;
  late Animation<double> _expandAnimation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 300),
    );
    _expandAnimation = CurvedAnimation(
      parent: _controller,
      curve: Curves.easeInOut,
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _toggleExpand() {
    setState(() {
      _isExpanded = !_isExpanded;
      if (_isExpanded) {
        _controller.forward();
      } else {
        _controller.reverse();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        GestureDetector(
          onTap: _toggleExpand,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Row(children: [
                const Text("Fee"),
                const SizedBox(width: 2),
                Icon(_isExpanded ? Icons.keyboard_arrow_up : Icons.keyboard_arrow_down,
                    size: 22, color: Colors.black)
              ]),
              AmountText(amount: widget.value)
            ],
          ),
        ),
        SizeTransition(
          sizeFactor: _expandAnimation,
          child: Container(
            padding: const EdgeInsets.only(top: 0.0, left: 8.0, right: 0),
            color: Colors.grey[100], // You can change this to match your design
            child: SingleChildScrollView(
              child: GestureDetector(
                onTap: _toggleExpand,
                child: Column(
                  children: [
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.orderMatchingFee,
                        label: 'Order matching fee'),
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.fundingTxFee,
                        label: 'Funding tx fee'),
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.channelFeeReserve,
                        label: 'Channel reserve fee')
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
